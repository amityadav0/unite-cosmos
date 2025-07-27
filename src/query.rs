use cosmwasm_std::{Deps, StdResult, Order};
use cw_storage_plus::Bound;
use crate::msg::{ConfigResponse, EscrowResponse, EscrowsResponse, EscrowByHashResponse, 
                 FactoryConfigResponse, EscrowAddressResponse, CreationRequestResponse, CreationRequestsResponse};
use crate::state::{CONFIG, ESCROWS, ESCROW_BY_HASH, FACTORY_CONFIG, 
                   ESCROW_CREATION_REQUESTS, compute_escrow_address};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        access_token: config.access_token.to_string(),
        rescue_delay: config.rescue_delay,
        factory: config.factory.to_string(),
    })
}

pub fn query_factory_config(deps: Deps) -> StdResult<FactoryConfigResponse> {
    let config = FACTORY_CONFIG.load(deps.storage)?;
    Ok(FactoryConfigResponse {
        owner: config.owner.to_string(),
        escrow_contract: config.escrow_contract.to_string(),
        access_token: config.access_token.to_string(),
        rescue_delay: config.rescue_delay,
        min_safety_deposit: config.min_safety_deposit,
        max_safety_deposit: config.max_safety_deposit,
        creation_fee: config.creation_fee,
    })
}

pub fn query_escrow(deps: Deps, escrow_id: u64) -> StdResult<EscrowResponse> {
    let escrow_state = ESCROWS.load(deps.storage, escrow_id)?;
    
    Ok(EscrowResponse {
        escrow_id,
        immutables: escrow_state.escrow_info.immutables,
        dst_complement: escrow_state.escrow_info.dst_complement,
        escrow_type: escrow_state.escrow_info.escrow_type,
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
                escrow_type: escrow_state.escrow_info.escrow_type,
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
    let escrow_id = ESCROW_BY_HASH.may_load(deps.storage, hash)?;
    Ok(EscrowByHashResponse { escrow_id })
}

pub fn query_address_of_escrow(
    deps: Deps, 
    order_hash: String, 
    hashlock: String, 
    salt: String
) -> StdResult<EscrowAddressResponse> {
    // Get factory address from config
    let factory_config = FACTORY_CONFIG.load(deps.storage)?;
    
    // Compute deterministic address
    let address = compute_escrow_address(
        &factory_config.escrow_contract,
        &order_hash,
        &hashlock,
        &salt,
    );
    
    Ok(EscrowAddressResponse { address })
}

pub fn query_creation_request(
    deps: Deps, 
    order_hash: String, 
    hashlock: String
) -> StdResult<CreationRequestResponse> {
    let request_key = format!("{order_hash}:{hashlock}");
    let request = ESCROW_CREATION_REQUESTS.may_load(deps.storage, request_key)?;
    Ok(CreationRequestResponse { request })
}

pub fn query_creation_requests(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<CreationRequestsResponse> {
    let limit = limit.unwrap_or(30) as usize;
    let start = start_after.map(Bound::exclusive);

    let requests: StdResult<Vec<crate::state::EscrowCreationRequest>> = ESCROW_CREATION_REQUESTS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, request) = item?;
            Ok(request)
        })
        .collect();

    Ok(CreationRequestsResponse {
        requests: requests?,
    })
} 