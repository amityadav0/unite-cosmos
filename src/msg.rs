use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use crate::state::{Immutables, PackedTimelocks, EscrowType};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub access_token: String,
    pub rescue_delay: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    // Direct escrow deployment with funding
    DeployEscrowWithFunding {
        order_hash: String,
        hashlock: String,
        maker: String,
        taker: String,
        token: String,
        amount: Uint128,
        safety_deposit: Uint128,
        timelocks: PackedTimelocks,
        dst_chain_id: String,
        dst_token: String,
        dst_amount: Uint128,
        escrow_type: EscrowType,
    },
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

#[cw_serde]
pub struct EscrowByHashResponse {
    pub escrow_id: Option<u64>,
} 