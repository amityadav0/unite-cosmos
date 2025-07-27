use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, StdResult, CosmosMsg, BankMsg, WasmMsg, Uint128, Addr,
    to_json_binary, coins,
};
use cw20::Cw20ExecuteMsg;
use sha2::{Sha256, Digest};

use crate::error::ContractError;
use crate::msg::{InstantiateMsg, ExecuteMsg};
use crate::state::{
    Config, CONFIG, ESCROWS, ESCROW_COUNTER, ESCROW_BY_HASH, EscrowState, EscrowInfo, 
    Immutables, PackedTimelocks, DstImmutablesComplement, TimelockStage
};

pub fn execute_instantiate(
    deps: DepsMut,
    info: MessageInfo,
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
        .add_attribute("access_token", msg.access_token))
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
    dst_chain_id: String,
    dst_token: String,
    dst_amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    // Only factory can create escrows
    if info.sender != config.factory {
        return Err(ContractError::Unauthorized {});
    }

    let maker_addr = deps.api.addr_validate(&maker)?;
    let taker_addr = deps.api.addr_validate(&taker)?;
    let token_addr = deps.api.addr_validate(&token)?;

    // Validate that the correct amount of funds was sent
    let total_required = amount + safety_deposit;
    let sent_amount = info.funds.iter()
        .find(|coin| coin.denom == "uatom")
        .map(|coin| coin.amount)
        .unwrap_or_default();
    
    if sent_amount != total_required {
        return Err(ContractError::InsufficientBalance {});
    }

    // Create immutables with current timestamp
    let deployed_at = env.block.time.seconds() as u32;
    let mut immutables = Immutables {
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

    let escrow_hash = immutables.hash();
    
    // Check if escrow already exists
    if ESCROW_BY_HASH.has(deps.storage, escrow_hash.clone()) {
        return Err(ContractError::EscrowAlreadyExists {});
    }

    // Get next escrow ID
    let escrow_id = ESCROW_COUNTER.load(deps.storage)? + 1;
    ESCROW_COUNTER.save(deps.storage, &escrow_id)?;

    // Create destination complement
    let dst_complement = DstImmutablesComplement {
        maker: maker_addr.clone(),
        amount: dst_amount,
        token: deps.api.addr_validate(&dst_token)?,
        safety_deposit,
        chain_id: dst_chain_id,
    };

    let escrow_info = EscrowInfo {
        immutables,
        dst_complement: Some(dst_complement),
        is_src: true,
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
        .add_attribute("escrow_hash", escrow_hash))
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
    secret: String,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound {})?;

    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive {});
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Verify caller is taker
    if info.sender != immutables.taker {
        return Err(ContractError::InvalidCaller {});
    }

    // Verify secret hash matches hashlock
    let secret_hash = sha2::Sha256::digest(secret.as_bytes());
    let secret_hash_hex = format!("{:x}", secret_hash);
    
    if secret_hash_hex != immutables.hashlock {
        return Err(ContractError::InvalidSecret {});
    }

    // Check timelock stage
    let current_time = env.block.time.seconds();
    let stage = if escrow_state.escrow_info.is_src {
        TimelockStage::SrcWithdrawal
    } else {
        TimelockStage::DstWithdrawal
    };

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::InvalidTime {});
    }

    // Transfer tokens
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        if immutables.token == Addr::unchecked("") {
            // Native token
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: immutables.taker.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            // CW20 token
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
        .add_attribute("method", "withdraw")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("secret", secret))
}

pub fn execute_cancel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound {})?;

    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive {});
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Verify caller is taker
    if info.sender != immutables.taker {
        return Err(ContractError::InvalidCaller {});
    }

    // Check timelock stage
    let current_time = env.block.time.seconds();
    let stage = if escrow_state.escrow_info.is_src {
        TimelockStage::SrcCancellation
    } else {
        TimelockStage::DstCancellation
    };

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::InvalidTime {});
    }

    // Transfer tokens back to appropriate party
    let mut messages: Vec<CosmosMsg> = vec![];

    if escrow_state.balance > Uint128::zero() {
        let recipient = if escrow_state.escrow_info.is_src {
            immutables.maker.clone()
        } else {
            immutables.taker.clone()
        };

        if immutables.token == Addr::unchecked("") {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient.to_string(),
                amount: coins(escrow_state.balance.u128(), "uatom"),
            }));
        } else {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: immutables.token.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: recipient.to_string(),
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
        .add_attribute("method", "cancel")
        .add_attribute("escrow_id", escrow_id.to_string()))
}

pub fn execute_rescue(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
    token: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound {})?;

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Verify caller is taker
    if info.sender != immutables.taker {
        return Err(ContractError::InvalidCaller {});
    }

    // Check rescue delay
    let config = CONFIG.load(deps.storage)?;
    let current_time = env.block.time.seconds();
    let rescue_start = immutables.timelocks.rescue_start(config.rescue_delay);
    
    if current_time < rescue_start {
        return Err(ContractError::RescueDelayNotMet {});
    }

    // Transfer tokens
    let mut messages: Vec<CosmosMsg> = vec![];

    if token == "" {
        // Native token
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(amount.u128(), "uatom"),
        }));
    } else {
        // CW20 token
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "rescue_funds")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("token", token)
        .add_attribute("amount", amount.to_string()))
} 