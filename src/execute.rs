use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, CosmosMsg, BankMsg, WasmMsg, Uint128, Addr,
    coins, to_json_binary,
};
use cw20::Cw20ExecuteMsg;
use sha2::{Sha256, Digest};

use crate::error::ContractError;
use crate::msg::InstantiateMsg;
use crate::state::{
    CONFIG, ESCROWS, TimelockStage, EscrowState, EscrowInfo, 
    Immutables, PackedTimelocks, DstImmutablesComplement, EscrowType, get_next_escrow_id
};

pub fn execute_instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Validate that the correct amount of funds was sent
    let total_required = msg.amount + msg.safety_deposit;
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
        order_hash: msg.order_hash.clone(),
        hashlock: msg.hashlock.clone(),
        maker: deps.api.addr_validate(&msg.maker)?,
        taker: deps.api.addr_validate(&msg.taker)?,
        token: if msg.token.is_empty() {
            Addr::unchecked("") // Native token
        } else {
            deps.api.addr_validate(&msg.token)?
        },
        amount: msg.amount,
        safety_deposit: msg.safety_deposit,
        timelocks: PackedTimelocks::new(
            deployed_at,
            msg.timelocks.get(TimelockStage::SrcWithdrawal),
            msg.timelocks.get(TimelockStage::SrcPublicWithdrawal),
            msg.timelocks.get(TimelockStage::SrcCancellation),
            msg.timelocks.get(TimelockStage::SrcPublicCancellation),
            msg.timelocks.get(TimelockStage::DstWithdrawal),
            msg.timelocks.get(TimelockStage::DstPublicWithdrawal),
            msg.timelocks.get(TimelockStage::DstCancellation),
        ),
    };

    // Validate immutables
    immutables.validate()?;

    // Get next escrow ID
    let escrow_id = get_next_escrow_id(deps.storage)?;

    // Create destination complement (only for source escrows)
    let dst_complement = if msg.escrow_type.is_source() {
        Some(DstImmutablesComplement {
            maker: deps.api.addr_validate(&msg.maker)?,
            amount: msg.dst_amount,
            token: deps.api.addr_validate(&msg.dst_token)?,
            safety_deposit: msg.safety_deposit,
            chain_id: msg.dst_chain_id,
        })
    } else {
        None
    };

    let escrow_info = EscrowInfo {
        immutables,
        dst_complement,
        escrow_type: msg.escrow_type,
        is_active: true,
        created_at: env.block.time,
    };

    let escrow_state = EscrowState {
        escrow_info,
        balance: msg.amount,
        native_balance: msg.safety_deposit,
    };

    // Save escrow (no hash mapping needed in hybrid approach)
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("escrow_type", format!("{:?}", msg.escrow_type))
        .add_attribute("amount", msg.amount.to_string())
        .add_attribute("safety_deposit", msg.safety_deposit.to_string()))
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
    let secret_hash_hex = format!("{secret_hash:x}");
    
    if secret_hash_hex != immutables.hashlock {
        return Err(ContractError::InvalidSecret {});
    }

    // Timelock validation: allow in both PRIVATE and PUBLIC withdrawal stages
    let current_time = env.block.time.seconds();
    let private_stage = TimelockStage::SrcWithdrawal;
    let public_stage = TimelockStage::SrcPublicWithdrawal;
    let in_private = immutables.timelocks.is_within_stage(current_time, private_stage);
    let in_public = immutables.timelocks.is_within_stage(current_time, public_stage);
    if !(in_private || in_public) {
        return Err(ContractError::TimelockNotExpired { 
            stage: "SrcWithdrawal or SrcPublicWithdrawal".to_string() 
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
    let secret_hash_hex = format!("{secret_hash:x}");
    
    if secret_hash_hex != immutables.hashlock {
        return Err(ContractError::InvalidSecret {});
    }

    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_withdrawal_stage();

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{stage:?}") 
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
            stage: format!("{stage:?}") 
        });
    }

    // Transfer tokens to maker (source behavior)
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
            stage: format!("{stage:?}") 
        });
    }

    // Transfer tokens to taker (destination behavior)
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

    // Access control: only access token holder can public withdraw
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.access_token { // TODO:FIX access token holder
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
            stage: format!("{stage:?}") 
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
        .add_attribute("method", "public_withdraw_src")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.taker.to_string()))
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

    // Access control: only access token holder can public withdraw
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
            stage: format!("{stage:?}") 
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
        .add_attribute("method", "public_withdraw_dst")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.maker.to_string()))
}

/// Source-specific public cancel function
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

    // Access control: only access token holder can public cancel
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
            reason: "Public cancellation not supported for this escrow type".to_string() 
        })?;

    if !immutables.timelocks.is_within_stage(current_time, stage) {
        return Err(ContractError::TimelockNotExpired { 
            stage: format!("{stage:?}") 
        });
    }

    // Transfer tokens to maker (source behavior)
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
        .add_attribute("method", "public_cancel_src")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", immutables.maker.to_string()))
}

/// Rescue function for emergency fund recovery
pub fn execute_rescue(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    escrow_id: u64,
) -> Result<Response, ContractError> {
    let mut escrow_state = ESCROWS.load(deps.storage, escrow_id)
        .map_err(|_| ContractError::EscrowNotFound { escrow_id })?;

    // State validation
    if !escrow_state.escrow_info.is_active {
        return Err(ContractError::EscrowNotActive { escrow_id });
    }

    // Access control: only taker can rescue funds
    if info.sender != escrow_state.escrow_info.immutables.taker {
        return Err(ContractError::OnlyTaker {});
    }

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Rescue delay validation
    let config = CONFIG.load(deps.storage)?;
    let current_time = env.block.time.seconds();
    
    if !immutables.timelocks.is_rescue_available(current_time, config.rescue_delay) {
        return Err(ContractError::TimelockNotExpired { 
            stage: "Rescue delay not expired".to_string() 
        });
    }

    // Transfer all funds to caller (taker)
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
    escrow_state.escrow_info.is_active = false;
    ESCROWS.save(deps.storage, escrow_id, &escrow_state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "rescue")
        .add_attribute("escrow_id", escrow_id.to_string())
        .add_attribute("recipient", info.sender.to_string()))
} 