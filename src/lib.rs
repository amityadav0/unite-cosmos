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
use crate::factory::{
    execute_deploy_escrow_with_funding
};
use crate::query::{query_config, query_escrow, query_escrows, query_escrow_by_hash};

pub mod contract;
pub mod error;
pub mod execute;
pub mod factory;
pub mod msg;
pub mod query;
pub mod state;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    execute_instantiate(deps, info, msg)
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Direct escrow deployment with funding
        ExecuteMsg::DeployEscrowWithFunding { 
            order_hash, hashlock, maker, taker, token, amount, safety_deposit, 
            timelocks, dst_chain_id, dst_token, dst_amount, escrow_type 
        } => execute_deploy_escrow_with_funding(
            deps, env, info, order_hash, hashlock, maker, taker, token, 
            amount, safety_deposit, timelocks, dst_chain_id, dst_token, dst_amount, escrow_type
        ),
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
        QueryMsg::Escrow { escrow_id } => to_json_binary(&query_escrow(deps, escrow_id)?),
        QueryMsg::Escrows { start_after, limit } => to_json_binary(&query_escrows(deps, start_after, limit)?),
        QueryMsg::EscrowByHash { hash } => to_json_binary(&query_escrow_by_hash(deps, hash)?),
    }
} 