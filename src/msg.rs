use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use crate::state::{PackedTimelocks, EscrowType};

#[cw_serde]
pub struct InstantiateMsg {
    pub order_hash: String,
    pub hashlock: String,
    pub maker: String,
    pub taker: String,
    pub token: String,
    pub amount: Uint128,
    pub safety_deposit: Uint128,
    pub timelocks: PackedTimelocks,
    pub dst_chain_id: String,
    pub dst_token: String,
    pub dst_amount: Uint128,
    pub escrow_type: EscrowType,
}

#[cw_serde]
pub enum ExecuteMsg {
    // Escrow operations
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
    Rescue {
        escrow_id: u64,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub escrow_id: u64,
    pub immutables: crate::state::Immutables,
    pub dst_complement: Option<crate::state::DstImmutablesComplement>,
    pub escrow_type: crate::state::EscrowType,
    pub is_active: bool,
    pub balance: Uint128,
    pub native_balance: Uint128,
    pub created_at: String,
}

#[cw_serde]
pub struct EscrowResponse {
    pub escrow_id: u64,
    pub immutables: crate::state::Immutables,
    pub dst_complement: Option<crate::state::DstImmutablesComplement>,
    pub escrow_type: crate::state::EscrowType,
    pub is_active: bool,
    pub balance: Uint128,
    pub native_balance: Uint128,
    pub created_at: String,
}

#[cw_serde]
pub struct EscrowsResponse {
    pub escrows: Vec<EscrowResponse>,
} 