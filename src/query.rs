use cosmwasm_std::{Deps, StdResult};
use crate::msg::{ConfigResponse};
use crate::state::{ESCROWS, ESCROW_COUNTER};

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    // Get the escrow ID (should be 1 since there's only one escrow per contract)
    let escrow_id = ESCROW_COUNTER.load(deps.storage)?;
    
    if escrow_id == 0 {
        return Err(cosmwasm_std::StdError::NotFound {
            kind: "No escrow deployed".to_string(),
        });
    }

    // Load the escrow
    let escrow_state = ESCROWS.load(deps.storage, 1)?;
    
    Ok(ConfigResponse {
        escrow_id: 1,
        immutables: escrow_state.escrow_info.immutables,
        dst_complement: escrow_state.escrow_info.dst_complement,
        escrow_type: escrow_state.escrow_info.escrow_type,
        is_active: escrow_state.escrow_info.is_active,
        balance: escrow_state.balance,
        native_balance: escrow_state.native_balance,
        created_at: escrow_state.escrow_info.created_at.to_string(),
    })
} 