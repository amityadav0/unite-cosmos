use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use crate::state::{EscrowCreationParams};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: String,
    pub access_token: String,
    pub rescue_delay: u64,
    pub factory: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    // Factory-specific operations
    CreateEscrow {
        params: EscrowCreationParams,
        salt: String, // For deterministic address generation
    },
    HandlePostInteraction {
        order_hash: String,
        hashlock: String,
        maker: String,
        taker: String,
        token: String,
        amount: Uint128,
        safety_deposit: Uint128,
        timelocks: crate::state::PackedTimelocks,
        dst_chain_id: String,
        dst_token: String,
        dst_amount: Uint128,
    },
    CancelCreationRequest {
        order_hash: String,
        hashlock: String,
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
    Rescue {
        escrow_id: u64,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(FactoryConfigResponse)]
    FactoryConfig {},
    #[returns(EscrowResponse)]
    Escrow { escrow_id: u64 },
    #[returns(EscrowsResponse)]
    Escrows { start_after: Option<u64>, limit: Option<u32> },
    #[returns(EscrowByHashResponse)]
    EscrowByHash { hash: String },
    #[returns(EscrowAddressResponse)]
    AddressOfEscrow { 
        order_hash: String, 
        hashlock: String, 
        salt: String 
    },
    #[returns(CreationRequestResponse)]
    CreationRequest { 
        order_hash: String, 
        hashlock: String 
    },
    #[returns(CreationRequestsResponse)]
    CreationRequests { 
        start_after: Option<String>, 
        limit: Option<u32> 
    },
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub access_token: String,
    pub rescue_delay: u64,
    pub factory: String,
}

#[cw_serde]
pub struct FactoryConfigResponse {
    pub owner: String,
    pub escrow_contract: String,
    pub access_token: String,
    pub rescue_delay: u64,
    pub min_safety_deposit: Uint128,
    pub max_safety_deposit: Uint128,
    pub creation_fee: Uint128,
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

#[cw_serde]
pub struct EscrowAddressResponse {
    pub address: String,
}

#[cw_serde]
pub struct CreationRequestResponse {
    pub request: Option<crate::state::EscrowCreationRequest>,
}

#[cw_serde]
pub struct CreationRequestsResponse {
    pub requests: Vec<crate::state::EscrowCreationRequest>,
} 