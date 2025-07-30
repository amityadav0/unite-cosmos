use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, Uint128, Addr, CosmosMsg, WasmMsg, to_json_binary,
    Binary, coins,
};
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::InstantiateMsg;
use crate::state::{
    Config, CONFIG, ESCROW_COUNTER, 
    EscrowState, EscrowInfo, Immutables, PackedTimelocks, DstImmutablesComplement, 
    TimelockStage, EscrowType, save_escrow, get_next_escrow_id,
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
        factory: deps.api.addr_validate(&msg.owner)?, // Use owner as factory for simplicity
    };

    CONFIG.save(deps.storage, &config)?;
    ESCROW_COUNTER.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("access_token", msg.access_token)
        .add_attribute("rescue_delay", msg.rescue_delay.to_string()))
}

/// Direct escrow deployment with funding in single transaction
/// This replaces the Ethereum factory pattern with a simpler CosmWasm approach
pub fn execute_deploy_escrow_with_funding(
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
    escrow_type: EscrowType,
) -> Result<Response, ContractError> {
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

    // Create immutables for escrow
    let deployed_at = env.block.time.seconds() as u32;
    let immutables = Immutables {
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
    
    // Check if escrow already exists by hash
    if crate::state::escrow_exists_by_hash(deps.storage, escrow_hash.clone()) {
        return Err(ContractError::EscrowAlreadyExists { hash: escrow_hash });
    }

    // Get next escrow ID
    let escrow_id = get_next_escrow_id(deps.storage)?;

    // Create destination complement (only for source escrows)
    let dst_complement = if escrow_type.is_source() {
        Some(DstImmutablesComplement {
            maker: deps.api.addr_validate(&maker)?,
            amount: dst_amount,
            token: deps.api.addr_validate(&dst_token)?,
            safety_deposit: safety_deposit,
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
    save_escrow(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_attribute("method", "deploy_escrow_with_funding")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("escrow_hash", escrow_hash)
        .add_attribute("escrow_type", format!("{:?}", escrow_type))
        .add_attribute("amount", amount.to_string())
        .add_attribute("safety_deposit", safety_deposit.to_string()))
} 