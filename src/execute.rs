use cosmwasm_std::{
    DepsMut, Env, MessageInfo, Response, CosmosMsg, BankMsg, WasmMsg, Uint128, Addr,
    coins, to_json_binary,
};
use cw20::Cw20ExecuteMsg;
use sha2::{Sha256, Digest};

use crate::error::ContractError;
use crate::msg::InstantiateMsg;
use crate::state::{
    Config, CONFIG, ESCROWS, ESCROW_COUNTER
};

// Import factory functions

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

    // Initialize regular config
    CONFIG.save(deps.storage, &config)?;
    ESCROW_COUNTER.save(deps.storage, &0u64)?;

    // Initialize factory config
    let factory_config = crate::state::FactoryConfig {
        owner: deps.api.addr_validate(&msg.owner)?,
        escrow_contract: deps.api.addr_validate(&msg.factory)?,
        access_token: deps.api.addr_validate(&msg.access_token)?,
        rescue_delay: msg.rescue_delay,
        min_safety_deposit: Uint128::new(100), // Default minimum
        max_safety_deposit: Uint128::new(10000), // Default maximum
        creation_fee: Uint128::new(10), // Default creation fee
    };

    crate::state::FACTORY_CONFIG.save(deps.storage, &factory_config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", msg.owner)
        .add_attribute("access_token", msg.access_token)
        .add_attribute("rescue_delay", msg.rescue_delay.to_string())
        .add_attribute("factory", msg.factory))
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

    // Timelock validation
    let current_time = env.block.time.seconds();
    let stage = escrow_state.escrow_info.escrow_type.get_withdrawal_stage();

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

    let immutables = &escrow_state.escrow_info.immutables;
    
    // Rescue delay validation
    let config = CONFIG.load(deps.storage)?;
    let current_time = env.block.time.seconds();
    
    if !immutables.timelocks.is_rescue_available(current_time, config.rescue_delay) {
        return Err(ContractError::TimelockNotExpired { 
            stage: "Rescue delay not expired".to_string() 
        });
    }

    // Transfer all funds to caller
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