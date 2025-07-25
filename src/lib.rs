use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult,
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG, ESCROWS, EscrowInfo};
use crate::execute::{execute_instantiate, execute_create_escrow, execute_withdraw, execute_cancel, execute_rescue};
use crate::query::{query_config, query_escrow, query_escrows, query_escrow_by_hash};

pub mod contract;
pub mod error;
pub mod execute;
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
        ExecuteMsg::CreateEscrow { 
            order_hash, 
            hashlock, 
            maker, 
            taker, 
            token, 
            amount, 
            safety_deposit, 
            timelocks,
            dst_chain_id,
            dst_token,
            dst_amount 
        } => execute_create_escrow(
            deps, 
            env, 
            info, 
            order_hash, 
            hashlock, 
            maker, 
            taker, 
            token, 
            amount, 
            safety_deposit, 
            timelocks,
            dst_chain_id,
            dst_token,
            dst_amount
        ),
        ExecuteMsg::Withdraw { 
            escrow_id, 
            secret 
        } => execute_withdraw(deps, env, info, escrow_id, secret),
        ExecuteMsg::Cancel { escrow_id } => execute_cancel(deps, env, info, escrow_id),
        ExecuteMsg::RescueFunds { 
            escrow_id, 
            token, 
            amount 
        } => execute_rescue(deps, env, info, escrow_id, token, amount),
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