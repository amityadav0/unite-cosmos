use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult,
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::execute::{
    execute_instantiate, 
    execute_withdraw_src, execute_withdraw_dst, execute_cancel_src, execute_cancel_dst,
    execute_public_withdraw_src, execute_public_withdraw_dst, execute_public_cancel_src,
    execute_rescue
};
use crate::query::{query_config};

pub mod contract;
pub mod error;
pub mod execute;
pub mod msg;
pub mod query;
pub mod state;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    execute_instantiate(deps, env, info, msg)
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Escrow operations
        ExecuteMsg::WithdrawSrc { escrow_id, secret } => 
            execute_withdraw_src(deps, env, info, escrow_id, secret),
        ExecuteMsg::CancelSrc { escrow_id } => 
            execute_cancel_src(deps, env, info, escrow_id),
        ExecuteMsg::PublicWithdrawSrc { escrow_id } => 
            execute_public_withdraw_src(deps, env, info, escrow_id),
        ExecuteMsg::PublicCancelSrc { escrow_id } => 
            execute_public_cancel_src(deps, env, info, escrow_id),
        ExecuteMsg::WithdrawDst { escrow_id, secret } => 
            execute_withdraw_dst(deps, env, info, escrow_id, secret),
        ExecuteMsg::CancelDst { escrow_id } => 
            execute_cancel_dst(deps, env, info, escrow_id),
        ExecuteMsg::PublicWithdrawDst { escrow_id } => 
            execute_public_withdraw_dst(deps, env, info, escrow_id),
        ExecuteMsg::Rescue { escrow_id } => 
            execute_rescue(deps, env, info, escrow_id),
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
    }
} 