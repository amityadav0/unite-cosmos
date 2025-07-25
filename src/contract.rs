use cosmwasm_std::{Deps, StdResult};
use crate::state::{CONFIG, ESCROWS, EscrowState};

/// Get the total number of active escrows
pub fn get_active_escrow_count(deps: Deps) -> StdResult<u64> {
    let mut count = 0u64;
    
    for result in ESCROWS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (_, escrow_state) = result?;
        if escrow_state.escrow_info.is_active {
            count += 1;
        }
    }
    
    Ok(count)
}

/// Validate that an address has access token
pub fn has_access_token(deps: Deps, address: &str) -> StdResult<bool> {
    let config = CONFIG.load(deps.storage)?;
    let addr = deps.api.addr_validate(address)?;
    
    // In a real implementation, you would check the CW20 balance here
    // For now, we'll return true if the address is valid
    Ok(addr == config.owner)
}

/// Get escrow statistics
pub fn get_escrow_stats(deps: Deps) -> StdResult<(u64, u64)> {
    let mut total_escrows = 0u64;
    let mut active_escrows = 0u64;
    
    for result in ESCROWS.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
        let (_, escrow_state) = result?;
        total_escrows += 1;
        if escrow_state.escrow_info.is_active {
            active_escrows += 1;
        }
    }
    
    Ok((total_escrows, active_escrows))
} 