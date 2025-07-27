use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, StdResult, CosmosMsg, BankMsg, WasmMsg, Uint128, Addr,
    coins, to_json_binary,
};
use cw20::Cw20ExecuteMsg;
use sha2::{Sha256, Digest};

use crate::error::ContractError;
use crate::msg::{InstantiateMsg, ExecuteMsg};
use crate::state::{
    Config, CONFIG, ESCROWS, ESCROW_COUNTER, ESCROW_BY_HASH, EscrowState, EscrowInfo, 
    Immutables, PackedTimelocks, DstImmutablesComplement, TimelockStage, EscrowType
};

pub fn execute_instantiate(
    deps: DepsMut,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        access_token: deps.api.addr_validate(&msg.access_token)?,
        rescue_delay: msg.rescue_delay,
        factory: deps.api.addr_validate(&msg.factory)?,
    };

    CONFIG.save(deps.storage, &config)?;
    ESCROW_COUNTER.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("access_token", msg.access_token)
        .add_attribute("rescue_delay", msg.rescue_delay.to_string())
        .add_attribute("factory", msg.factory))
}

pub fn execute_create_escrow(
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
    escrow_type: EscrowType,
    dst_chain_id: String,
    dst_token: String,
    dst_amount: Uint128,
) -> Result<Response, ContractError> {
    // Access control: only factory can create escrows
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.factory {
        return Err(ContractError::OnlyAccessTokenHolder {});
    }

    // Validate inputs
    if order_hash.is_empty() || hashlock.is_empty() {
        return Err(ContractError::InvalidImmutables { 
            reason: "Order hash and hashlock cannot be empty".to_string() 
        });
    }

    if amount == Uint128::zero() || safety_deposit == Uint128::zero() {
        return Err(ContractError::InvalidAmount { 
            amount: "Amount and safety deposit must be greater than zero".to_string() 
        });
    }

    // Validate addresses
    let maker_addr = deps.api.addr_validate(&maker)?;
    let taker_addr = deps.api.addr_validate(&taker)?;
    let token_addr = if token.is_empty() {
        Addr::unchecked("") // Native token
    } else {
        deps.api.addr_validate(&token)?
    };

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

    // Create immutables with current timestamp
    let deployed_at = env.block.time.seconds() as u32;
    let immutables = Immutables {
        order_hash,
        hashlock,
        maker: maker_addr.clone(),
        taker: taker_addr,
        token: token_addr,
        amount,
        safety_deposit,
        timelocks: PackedTimelocks::new(
            deployed_at,
            timelocks.get(TimelockStage::SrcWithdrawal),
            timelocks.get(TimelockStage::SrcPublicWithdrawal),
            timelocks.get(TimelockStage::SrcCancellation),
            timelocks.get(TimelockStage::SrcPublicCancellation),
            timelocks.get(TimelockStage::DstWithdrawal),
            timelocks.get(TimelockStage::DstPublicWithdrawal),
            timelocks.get(TimelockStage::DstCancellation),
        ),
    };

    // Validate immutables
    immutables.validate()?;

    let escrow_hash = immutables.hash();
    
    // Check if escrow already exists
    if ESCROW_BY_HASH.has(deps.storage, escrow_hash.clone()) {
        return Err(ContractError::EscrowAlreadyExists { hash: escrow_hash });
    }

    // Get next escrow ID
    let escrow_id = ESCROW_COUNTER.load(deps.storage)? + 1;
    ESCROW_COUNTER.save(deps.storage, &escrow_id)?;

    // Create destination complement (only for source escrows)
    let dst_complement = if escrow_type.is_source() {
        Some(DstImmutablesComplement {
            maker: maker_addr.clone(),
            amount: dst_amount,
            token: deps.api.addr_validate(&dst_token)?,
            safety_deposit,
            chain_id: dst_chain_id,
        })
    } else {
        None
    };

    let escrow_info = EscrowInfo {
        immutables,
        dst_complement,
        escrow_type,
        is_active: true,
        created_at: env.block.time,
    };

    let escrow_state = EscrowState {
        escrow_info,
        balance: amount,
        native_balance: safety_deposit,
    };

    // Save escrow
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;
    ESCROW_BY_HASH.save(deps.storage, escrow_hash.clone(), &escrow_id)?;

    Ok(Response::new()
        .add_attribute("method", "create_escrow")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("escrow_hash", escrow_hash)
        .add_attribute("escrow_type", format!("{:?}", escrow_type)))
}

/// Source-specific withdraw function
pub fn execute_withdraw_src(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
    secret: String,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Validate escrow type
    if !escrow_state.escrow_info.escrow_type.is_source() {
        return Err(ContractError::InvalidImmutables { 
            reason: "This operation is only valid for source escrows".to_string() 
        });
    }

    // Access control: only taker can withdraw
    if info.sender != escrow_state.escrow_info.immutables.taker {
        return Err(ContractError::OnlyTaker {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Secret validation
    let secret_hash = Sha256::digest(secret.as_bytes());
    let secret_hash_hex = format!("{:x}", secret_hash);
    
    if secret_hash_hex != immutables.hashlock {
        return Err(ContractError::InvalidSecret {});
    }

    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_withdrawal_stage();

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{:?}", stage) 
        });
    }

    // Transfer tokens to taker (source behavior)
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: immutables.taker.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: immutables.taker.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    // Transfer safety deposit to caller
    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "withdraw_src")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.taker.to_string())
        .add_attribute("secret", secret))
}

/// Destination-specific withdraw function
pub fn execute_withdraw_dst(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
    secret: String,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Validate escrow type
    if !escrow_state.escrow_info.escrow_type.is_destination() {
        return Err(ContractError::InvalidImmutables { 
            reason: "This operation is only valid for destination escrows".to_string() 
        });
    }

    // Access control: only taker can withdraw
    if info.sender != escrow_state.escrow_info.immutables.taker {
        return Err(ContractError::OnlyTaker {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Secret validation
    let secret_hash = Sha256::digest(secret.as_bytes());
    let secret_hash_hex = format!("{:x}", secret_hash);
    
    if secret_hash_hex != immutables.hashlock {
        return Err(ContractError::InvalidSecret {});
    }

    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_withdrawal_stage();

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{:?}", stage) 
        });
    }

    // Transfer tokens to maker (destination behavior)
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: immutables.maker.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: immutables.maker.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    // Transfer safety deposit to caller
    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "withdraw_dst")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.maker.to_string())
        .add_attribute("secret", secret))
}

/// Source-specific cancel function
pub fn execute_cancel_src(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Validate escrow type
    if !escrow_state.escrow_info.escrow_type.is_source() {
        return Err(ContractError::InvalidImmutables { 
            reason: "This operation is only valid for source escrows".to_string() 
        });
    }

    // Access control: only taker can cancel
    if info.sender != escrow_state.escrow_info.immutables.taker {
        return Err(ContractError::OnlyTaker {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_cancellation_stage();

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{:?}", stage) 
        });
    }

    // Transfer tokens back to maker (source behavior)
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: immutables.maker.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: immutables.maker.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    // Transfer safety deposit to caller
    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "cancel_src")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.maker.to_string()))
}

/// Destination-specific cancel function
pub fn execute_cancel_dst(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Validate escrow type
    if !escrow_state.escrow_info.escrow_type.is_destination() {
        return Err(ContractError::InvalidImmutables { 
            reason: "This operation is only valid for destination escrows".to_string() 
        });
    }

    // Access control: only taker can cancel
    if info.sender != escrow_state.escrow_info.immutables.taker {
        return Err(ContractError::OnlyTaker {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_cancellation_stage();

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{:?}", stage) 
        });
    }

    // Transfer tokens back to taker (destination behavior)
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: immutables.taker.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: immutables.taker.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    // Transfer safety deposit to caller
    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "cancel_dst")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.taker.to_string()))
}

/// Source-specific public withdraw function
pub fn execute_public_withdraw_src(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Validate escrow type
    if !escrow_state.escrow_info.escrow_type.is_source() {
        return Err(ContractError::InvalidImmutables { 
            reason: "This operation is only valid for source escrows".to_string() 
        });
    }

    // Access control: only access token holder can execute
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.access_token {
        return Err(ContractError::OnlyAccessTokenHolder {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_public_withdrawal_stage();

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{:?}", stage) 
        });
    }

    // Transfer tokens to access token holder
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    // Transfer safety deposit to access token holder
    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "public_withdraw_src")
        .add_attribute("escrow_id", escrow_id.to_string()))
}

/// Destination-specific public withdraw function
pub fn execute_public_withdraw_dst(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Validate escrow type
    if !escrow_state.escrow_info.escrow_type.is_destination() {
        return Err(ContractError::InvalidImmutables { 
            reason: "This operation is only valid for destination escrows".to_string() 
        });
    }

    // Access control: only access token holder can execute
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.access_token {
        return Err(ContractError::OnlyAccessTokenHolder {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_public_withdrawal_stage();

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{:?}", stage) 
        });
    }

    // Transfer tokens to access token holder
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    // Transfer safety deposit to access token holder
    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "public_withdraw_dst")
        .add_attribute("escrow_id", escrow_id.to_string()))
}

/// Source-specific public cancel function (destination has no public cancel)
pub fn execute_public_cancel_src(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Validate escrow type
    if !escrow_state.escrow_info.escrow_type.is_source() {
        return Err(ContractError::InvalidImmutables { 
            reason: "This operation is only valid for source escrows".to_string() 
        });
    }

    // Access control: only access token holder can execute
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.access_token {
        return Err(ContractError::OnlyAccessTokenHolder {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_public_cancellation_stage()
        .ok_or_else(|| ContractError::InvalidImmutables { 
            reason: "Source escrow must support public cancellation".to_string() 
        })?;

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{:?}", stage) 
        });
    }

    // Transfer tokens back to maker
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: immutables.maker.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: immutables.maker.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    // Transfer safety deposit to access token holder
    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "public_cancel_src")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.maker.to_string()))
}

// Legacy functions for backward compatibility
pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
    secret: String,
) -> Result<Response, ContractError> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Route to appropriate function based on escrow type
    match escrow_state.escrow_info.escrow_type {
        EscrowType::Source => execute_withdraw_src(deps, env, info, escrow_id, secret),
        EscrowType::Destination => execute_withdraw_dst(deps, env, info, escrow_id, secret),
    }
}

pub fn execute_cancel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Route to appropriate function based on escrow type
    match escrow_state.escrow_info.escrow_type {
        EscrowType::Source => execute_cancel_src(deps, env, info, escrow_id),
        EscrowType::Destination => execute_cancel_dst(deps, env, info, escrow_id),
    }
}

pub fn execute_public_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Route to appropriate function based on escrow type
    match escrow_state.escrow_info.escrow_type {
        EscrowType::Source => execute_public_withdraw_src(deps, env, info, escrow_id),
        EscrowType::Destination => execute_public_withdraw_dst(deps, env, info, escrow_id),
    }
}

pub fn execute_public_cancel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Route to appropriate function based on escrow type
    match escrow_state.escrow_info.escrow_type {
        EscrowType::Source => execute_public_cancel_src(deps, env, info, escrow_id),
        EscrowType::Destination => {
            Err(ContractError::InvalidImmutables { 
                reason: "Destination escrows do not support public cancellation".to_string() 
            })
        }
    }
}

/// Rescue function - emergency fund recovery after delay
pub fn execute_rescue(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // Access control: only taker can rescue
    if info.sender != escrow_state.escrow_info.immutables.taker {
        return Err(ContractError::OnlyTaker {});
    }

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Check rescue delay
    let config = CONFIG.load(deps.storage)?;
    let current_time = env.block.time.seconds();
    let rescue_start = immutables.timelocks.rescue_start(config.rescue_delay);
    
    if current_time < rescue_start {
        return Err(ContractError::RescueDelayNotMet { 
            current: current_time, 
            required: rescue_start 
        });
    }

    // Transfer all remaining funds to taker
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: info.sender.to_string(),
                    amount: escrow_state.balance,
                })?,
                funds: vec![],
            }));
        }
    }

    if escrow_state.native_balance > Uint128::zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(escrow_state.native_balance.u128(), "uatom"),
        }));
    }

    // Mark escrow as inactive
    let mut updated_escrow = escrow_state;
    updated_escrow.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &updated_escrow)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "rescue")
        .add_attribute("escrow_id", escrow_id.to_string()))
} 