use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Timestamp};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub access_token: Addr,
    pub rescue_delay: u64,
    pub factory: Addr,
}

#[cw_serde]
pub struct Timelocks {
    pub src_withdrawal: u64,
    pub src_public_withdrawal: u64,
    pub src_cancellation: u64,
    pub src_public_cancellation: u64,
    pub dst_withdrawal: u64,
    pub dst_public_withdrawal: u64,
    pub dst_cancellation: u64,
    pub deployed_at: u64,
}

#[cw_serde]
pub struct Immutables {
    pub order_hash: String,
    pub hashlock: String,
    pub maker: Addr,
    pub taker: Addr,
    pub token: Addr,
    pub amount: Uint128,
    pub safety_deposit: Uint128,
    pub timelocks: Timelocks,
}

#[cw_serde]
pub struct DstImmutablesComplement {
    pub maker: Addr,
    pub amount: Uint128,
    pub token: Addr,
    pub safety_deposit: Uint128,
    pub chain_id: String,
}

#[cw_serde]
pub struct EscrowInfo {
    pub immutables: Immutables,
    pub dst_complement: Option<DstImmutablesComplement>,
    pub is_src: bool,
    pub is_active: bool,
    pub created_at: Timestamp,
}

#[cw_serde]
pub struct EscrowState {
    pub escrow_info: EscrowInfo,
    pub balance: Uint128,
    pub native_balance: Uint128,
}

// Storage keys
pub const CONFIG: Item<Config> = Item::new("config");
pub const ESCROWS: Map<u64, EscrowState> = Map::new("escrows");
pub const ESCROW_COUNTER: Item<u64> = Item::new("escrow_counter");
pub const ESCROW_BY_HASH: Map<String, u64> = Map::new("escrow_by_hash");

impl Timelocks {
    pub fn get_stage_time(&self, stage: TimelockStage) -> u64 {
        match stage {
            TimelockStage::SrcWithdrawal => self.deployed_at + self.src_withdrawal,
            TimelockStage::SrcPublicWithdrawal => self.deployed_at + self.src_public_withdrawal,
            TimelockStage::SrcCancellation => self.deployed_at + self.src_cancellation,
            TimelockStage::SrcPublicCancellation => self.deployed_at + self.src_public_cancellation,
            TimelockStage::DstWithdrawal => self.deployed_at + self.dst_withdrawal,
            TimelockStage::DstPublicWithdrawal => self.deployed_at + self.dst_public_withdrawal,
            TimelockStage::DstCancellation => self.deployed_at + self.dst_cancellation,
        }
    }

    pub fn is_within_stage(&self, current_time: u64, stage: TimelockStage) -> bool {
        let stage_time = self.get_stage_time(stage);
        current_time >= stage_time
    }

    pub fn rescue_start(&self, rescue_delay: u64) -> u64 {
        self.deployed_at + rescue_delay
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TimelockStage {
    SrcWithdrawal,
    SrcPublicWithdrawal,
    SrcCancellation,
    SrcPublicCancellation,
    DstWithdrawal,
    DstPublicWithdrawal,
    DstCancellation,
}

impl Immutables {
    pub fn hash(&self) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(self.order_hash.as_bytes());
        hasher.update(self.hashlock.as_bytes());
        hasher.update(self.maker.as_str().as_bytes());
        hasher.update(self.taker.as_str().as_bytes());
        hasher.update(self.token.as_str().as_bytes());
        hasher.update(self.amount.to_string().as_bytes());
        hasher.update(self.safety_deposit.to_string().as_bytes());
        hasher.update(self.timelocks.src_withdrawal.to_string().as_bytes());
        hasher.update(self.timelocks.src_public_withdrawal.to_string().as_bytes());
        hasher.update(self.timelocks.src_cancellation.to_string().as_bytes());
        hasher.update(self.timelocks.src_public_cancellation.to_string().as_bytes());
        hasher.update(self.timelocks.dst_withdrawal.to_string().as_bytes());
        hasher.update(self.timelocks.dst_public_withdrawal.to_string().as_bytes());
        hasher.update(self.timelocks.dst_cancellation.to_string().as_bytes());
        hasher.update(self.timelocks.deployed_at.to_string().as_bytes());
        
        format!("{:x}", hasher.finalize())
    }
} 