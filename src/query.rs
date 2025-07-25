use cosmwasm_std::{Deps, StdResult, Order};
use cw_storage_plus::Bound;

use crate::msg::{ConfigResponse, EscrowResponse, EscrowsResponse, EscrowByHashResponse};
use crate::state::{CONFIG, ESCROWS, ESCROW_BY_HASH, EscrowState};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        access_token: config.access_token.to_string(),
        rescue_delay: config.rescue_delay,
        factory: config.factory.to_string(),
    })
}

pub fn query_escrow(deps: Deps, escrow_id: u64) -> StdResult<EscrowResponse> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)?;
    
    Ok(EscrowResponse {
        escrow_id,
        immutables: escrow_state.escrow_info.immutables,
        dst_complement: escrow_state.escrow_info.dst_complement,
        is_src: escrow_state.escrow_info.is_src,
        is_active: escrow_state.escrow_info.is_active,
        balance: escrow_state.balance,
        native_balance: escrow_state.native_balance,
        created_at: escrow_state.escrow_info.created_at.to_string(),
    })
}

pub fn query_escrows(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<EscrowsResponse> {
    let limit = limit.unwrap_or(30) as usize;
    let start = start_after.map(Bound::exclusive);

    let escrows: StdResult<Vec<EscrowResponse>> = ESCROWS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (escrow_id, escrow_state) = item?;
            Ok(EscrowResponse {
                escrow_id,
                immutables: escrow_state.escrow_info.immutables,
                dst_complement: escrow_state.escrow_info.dst_complement,
                is_src: escrow_state.escrow_info.is_src,
                is_active: escrow_state.escrow_info.is_active,
                balance: escrow_state.balance,
                native_balance: escrow_state.native_balance,
                created_at: escrow_state.escrow_info.created_at.to_string(),
            })
        })
        .collect();

    Ok(EscrowsResponse {
        escrows: escrows?,
    })
}

pub fn query_escrow_by_hash(deps: Deps, hash: String) -> StdResult<EscrowByHashResponse> {
    let escrow_id = ESCROW_BY_HASH.may_load(deps.storage, &hash)?;
    Ok(EscrowByHashResponse { escrow_id })
} 