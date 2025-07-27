use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use crate::state::{Immutables, PackedTimelocks, DstImmutablesComplement, EscrowType};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub access_token: String,
    pub rescue_delay: u64,
    pub factory: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    CreateEscrow {
        order_hash: String,
        hashlock: String,
        maker: String,
        taker: String,
        token: String,
        amount: Uint128,
        safety_deposit: Uint128,
        timelocks: PackedTimelocks,
        escrow_type: EscrowType, // Source or Destination
        dst_chain_id: String,
        dst_token: String,
        dst_amount: Uint128,
    },
    // Source-specific operations
    WithdrawSrc {
        escrow_id: u64,
        secret: String,
    },
    CancelSrc {
        escrow_id: u64,
    },
    PublicWithdrawSrc {
        escrow_id: u64,
    },
    PublicCancelSrc {
        escrow_id: u64,
    },
    // Destination-specific operations
    WithdrawDst {
        escrow_id: u64,
        secret: String,
    },
    CancelDst {
        escrow_id: u64,
    },
    PublicWithdrawDst {
        escrow_id: u64,
    },
    // Generic operations (for backward compatibility)
    Withdraw {
        escrow_id: u64,
        secret: String,
    },
    Cancel {
        escrow_id: u64,
    },
    PublicWithdraw {
        escrow_id: u64,
    },
    PublicCancel {
        escrow_id: u64,
    },
    Rescue {
        escrow_id: u64,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(EscrowResponse)]
    Escrow { escrow_id: u64 },
    #[returns(EscrowsResponse)]
    Escrows { start_after: Option<u64>, limit: Option<u32> },
    #[returns(EscrowByHashResponse)]
    EscrowByHash { hash: String },
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub access_token: String,
    pub rescue_delay: u64,
    pub factory: String,
}

#[cw_serde]
pub struct EscrowResponse {
    pub escrow_id: u64,
    pub immutables: Immutables,
    pub dst_complement: Option<DstImmutablesComplement>,
    pub escrow_type: EscrowType,
    pub is_active: bool,
    pub balance: Uint128,
    pub native_balance: Uint128,
    pub created_at: String,
}

#[cw_serde]
pub struct EscrowsResponse {
    pub escrows: Vec<EscrowResponse>,
}

#[cw_serde]
pub struct EscrowByHashResponse {
    pub escrow_id: Option<u64>,
} 