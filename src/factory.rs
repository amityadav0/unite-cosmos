use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, Uint128, Addr,
};

use crate::error::ContractError;
use crate::msg::InstantiateMsg;
use crate::state::{
    FactoryConfig, EscrowCreationParams, EscrowCreationRequest, CreationStatus,
    FACTORY_CONFIG, ESCROW_CREATION_REQUESTS, ESCROW_ADDRESSES,
    validate_escrow_creation, compute_escrow_address, ESCROW_COUNTER, EscrowState, EscrowInfo, 
    Immutables, PackedTimelocks, DstImmutablesComplement, TimelockStage, EscrowType
};

pub fn execute_instantiate_factory(
    deps: DepsMut,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let factory_config = FactoryConfig {
        owner: deps.api.addr_validate(&msg.owner)?,
        escrow_contract: deps.api.addr_validate(&msg.factory)?, // Factory address as escrow contract
        access_token: deps.api.addr_validate(&msg.access_token)?,
        rescue_delay: msg.rescue_delay,
        min_safety_deposit: Uint128::new(100), // Default minimum
        max_safety_deposit: Uint128::new(10000), // Default maximum
        creation_fee: Uint128::new(10), // Default creation fee
    };

    FACTORY_CONFIG.save(deps.storage, &factory_config)?;
    ESCROW_COUNTER.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate_factory")
        .add_attribute("owner", msg.owner)
        .add_attribute("escrow_contract", msg.factory)
        .add_attribute("access_token", msg.access_token)
        .add_attribute("rescue_delay", msg.rescue_delay.to_string()))
}

/// Create new escrow instance with deterministic address
pub fn execute_create_escrow(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    params: EscrowCreationParams,
    salt: String,
) -> Result<Response, ContractError> {
    // Load factory configuration
    let factory_config = FACTORY_CONFIG.load(deps.storage)?;

    // Validate creation parameters
    validate_escrow_creation(&params, &factory_config)?;

    // Check creation fee
    let sent_fee = info.funds.iter()
        .find(|coin| coin.denom == "uatom")
        .map(|coin| coin.amount)
        .unwrap_or_default();

    if sent_fee < factory_config.creation_fee {
        return Err(ContractError::InsufficientBalance { 
            required: factory_config.creation_fee.to_string(), 
            available: sent_fee.to_string() 
        });
    }

    // Generate deterministic escrow address
    let escrow_address = compute_escrow_address(
        &env.contract.address,
        &params.order_hash,
        &params.hashlock,
        &salt,
    );

    // Check if escrow already exists
    if ESCROW_ADDRESSES.has(deps.storage, escrow_address.clone()) {
        return Err(ContractError::EscrowAlreadyExists { hash: escrow_address });
    }

    // Create escrow creation request
    let creation_request = EscrowCreationRequest {
        params: params.clone(),
        created_at: env.block.time,
        status: CreationStatus::Pending,
        escrow_address: Some(Addr::unchecked(escrow_address.clone())),
    };

    // Save creation request
    let request_key = format!("{}:{}", params.order_hash, params.hashlock);
    ESCROW_CREATION_REQUESTS.save(deps.storage, request_key.clone(), &creation_request)?;

    // Save escrow address mapping
    ESCROW_ADDRESSES.save(deps.storage, escrow_address.clone(), &env.contract.address)?;

    // Create immutables for escrow with proper deployed_at timestamp
    let deployed_at = env.block.time.seconds() as u32;
    let immutables = Immutables {
        order_hash: params.order_hash.clone(),
        hashlock: params.hashlock.clone(),
        maker: params.maker.clone(),
        taker: params.taker.clone(),
        token: params.token.clone(),
        amount: params.amount,
        safety_deposit: params.safety_deposit,
        timelocks: PackedTimelocks::new(
            deployed_at,
            params.timelocks.get(TimelockStage::SrcWithdrawal),
            params.timelocks.get(TimelockStage::SrcPublicWithdrawal),
            params.timelocks.get(TimelockStage::SrcCancellation),
            params.timelocks.get(TimelockStage::SrcPublicCancellation),
            params.timelocks.get(TimelockStage::DstWithdrawal),
            params.timelocks.get(TimelockStage::DstPublicWithdrawal),
            params.timelocks.get(TimelockStage::DstCancellation),
        ),
    };

    // Validate immutables
    immutables.validate()?;

    let escrow_hash = immutables.hash();
    
    // Check if escrow already exists by hash
    if crate::state::escrow_exists_by_hash(deps.storage, escrow_hash.clone()) {
        return Err(ContractError::EscrowAlreadyExists { hash: escrow_hash });
    }

    // Get next escrow ID
    let escrow_id = crate::state::get_next_escrow_id(deps.storage)?;

    // Create destination complement (only for source escrows)
    let dst_complement = if params.escrow_type.is_source() {
        Some(DstImmutablesComplement {
            maker: params.maker.clone(),
            amount: params.dst_amount,
            token: params.dst_token.clone(),
            safety_deposit: params.safety_deposit,
            chain_id: params.dst_chain_id,
        })
    } else {
        None
    };

    let escrow_info = EscrowInfo {
        immutables,
        dst_complement,
        escrow_type: params.escrow_type,
        is_active: true,
        created_at: env.block.time,
    };

    let escrow_state = EscrowState {
        escrow_info,
        balance: params.amount,
        native_balance: params.safety_deposit,
    };

    // Save escrow
    crate::state::save_escrow(deps.storage, escrow_id, &escrow_state)?;

    // Update creation request status
    let mut updated_request = creation_request;
    updated_request.status = CreationStatus::Created;
    ESCROW_CREATION_REQUESTS.save(deps.storage, request_key, &updated_request)?;

    Ok(Response::new()
        .add_attribute("method", "create_escrow")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("escrow_hash", escrow_hash)
        .add_attribute("escrow_address", escrow_address)
        .add_attribute("escrow_type", format!("{:?}", params.escrow_type))
        .add_attribute("salt", salt))
}

/// Handle post-interaction escrow creation (for source escrows)
pub fn execute_handle_post_interaction(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    order_hash: String,
    hashlock: String,
    maker: String,
    taker: String,
    token: String,
    amount: Uint128,
    safety_deposit: Uint128,
    timelocks: PackedTimelocks,
    dst_chain_id: String,
    dst_token: String,
    dst_amount: Uint128,
) -> Result<Response, ContractError> {
    // Load factory configuration
    let factory_config = FACTORY_CONFIG.load(deps.storage)?;

    // Create escrow parameters for source escrow
    let params = EscrowCreationParams {
        order_hash: order_hash.clone(),
        hashlock: hashlock.clone(),
        maker: deps.api.addr_validate(&maker)?,
        taker: deps.api.addr_validate(&taker)?,
        token: if token.is_empty() {
            Addr::unchecked("") // Native token
        } else {
            deps.api.addr_validate(&token)?
        },
        amount,
        safety_deposit,
        timelocks,
        escrow_type: EscrowType::Source, // Always source for post-interaction
        dst_chain_id,
        dst_token: deps.api.addr_validate(&dst_token)?,
        dst_amount,
    };

    // Validate creation parameters
    validate_escrow_creation(&params, &factory_config)?;

    // Validate that the correct amount of funds was sent
    let total_required = amount + safety_deposit;
    let sent_amount = info.funds.iter()
        .find(|coin| coin.denom == "uatom")
        .map(|coin| coin.amount)
        .unwrap_or_default();

    if sent_amount != total_required {
        return Err(ContractError::InsufficientBalance { 
            required: total_required.to_string(), 
            available: sent_amount.to_string() 
        });
    }

    // Generate deterministic escrow address using order hash as salt
    let salt = format!("post_interaction_{order_hash}");
    let escrow_address = compute_escrow_address(
        &env.contract.address,
        &params.order_hash,
        &params.hashlock,
        &salt,
    );

    // Check if escrow already exists
    if ESCROW_ADDRESSES.has(deps.storage, escrow_address.clone()) {
        return Err(ContractError::EscrowAlreadyExists { hash: escrow_address });
    }

    // Create escrow creation request
    let creation_request = EscrowCreationRequest {
        params: params.clone(),
        created_at: env.block.time,
        status: CreationStatus::Pending,
        escrow_address: Some(Addr::unchecked(escrow_address.clone())),
    };

    // Save creation request
    let request_key = format!("{}:{}", params.order_hash, params.hashlock);
    ESCROW_CREATION_REQUESTS.save(deps.storage, request_key.clone(), &creation_request)?;

    // Save escrow address mapping
    ESCROW_ADDRESSES.save(deps.storage, escrow_address.clone(), &env.contract.address)?;

    // Create immutables for escrow with proper deployed_at timestamp
    let deployed_at = env.block.time.seconds() as u32;
    let immutables = Immutables {
        order_hash: params.order_hash.clone(),
        hashlock: params.hashlock.clone(),
        maker: params.maker.clone(),
        taker: params.taker.clone(),
        token: params.token.clone(),
        amount: params.amount,
        safety_deposit: params.safety_deposit,
        timelocks: PackedTimelocks::new(
            deployed_at,
            params.timelocks.get(TimelockStage::SrcWithdrawal),
            params.timelocks.get(TimelockStage::SrcPublicWithdrawal),
            params.timelocks.get(TimelockStage::SrcCancellation),
            params.timelocks.get(TimelockStage::SrcPublicCancellation),
            params.timelocks.get(TimelockStage::DstWithdrawal),
            params.timelocks.get(TimelockStage::DstPublicWithdrawal),
            params.timelocks.get(TimelockStage::DstCancellation),
        ),
    };

    // Validate immutables
    immutables.validate()?;

    let escrow_hash = immutables.hash();
    
    // Check if escrow already exists by hash
    if crate::state::escrow_exists_by_hash(deps.storage, escrow_hash.clone()) {
        return Err(ContractError::EscrowAlreadyExists { hash: escrow_hash });
    }

    // Get next escrow ID
    let escrow_id = crate::state::get_next_escrow_id(deps.storage)?;

    // Create destination complement for source escrow
    let dst_complement = DstImmutablesComplement {
        maker: params.maker.clone(),
        amount: params.dst_amount,
        token: params.dst_token.clone(),
        safety_deposit: params.safety_deposit,
        chain_id: params.dst_chain_id,
    };

    let escrow_info = EscrowInfo {
        immutables,
        dst_complement: Some(dst_complement),
        escrow_type: EscrowType::Source,
        is_active: true,
        created_at: env.block.time,
    };

    let escrow_state = EscrowState {
        escrow_info,
        balance: params.amount,
        native_balance: params.safety_deposit,
    };

    // Save escrow
    crate::state::save_escrow(deps.storage, escrow_id, &escrow_state)?;

    // Update creation request status
    let mut updated_request = creation_request;
    updated_request.status = CreationStatus::Created;
    ESCROW_CREATION_REQUESTS.save(deps.storage, request_key, &updated_request)?;

    Ok(Response::new()
        .add_attribute("method", "handle_post_interaction")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("escrow_hash", escrow_hash)
        .add_attribute("escrow_address", escrow_address)
        .add_attribute("escrow_type", "Source"))
}

/// Cancel escrow creation request
pub fn execute_cancel_creation_request(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    order_hash: String,
    hashlock: String,
) -> Result<Response, ContractError> {
    // Load factory configuration
    let factory_config = FACTORY_CONFIG.load(deps.storage)?;

    // Only owner can cancel creation requests
    if info.sender != factory_config.owner {
        return Err(ContractError::Unauthorized { 
            reason: "Only factory owner can cancel creation requests".to_string() 
        });
    }

    // Load creation request
    let request_key = format!("{order_hash}:{hashlock}");
    let mut creation_request = ESCROW_CREATION_REQUESTS.load(deps.storage, request_key.clone())
        .map_err(|_| ContractError::EscrowNotFound { escrow_id: 0 })?;

    // Check if request can be cancelled
    if creation_request.status != CreationStatus::Pending {
        return Err(ContractError::InvalidImmutables { 
            reason: "Only pending requests can be cancelled".to_string() 
        });
    }

    // Update request status
    creation_request.status = CreationStatus::Cancelled;
    ESCROW_CREATION_REQUESTS.save(deps.storage, request_key, &creation_request)?;

    Ok(Response::new()
        .add_attribute("method", "cancel_creation_request")
        .add_attribute("order_hash", order_hash)
        .add_attribute("hashlock", hashlock))
} 