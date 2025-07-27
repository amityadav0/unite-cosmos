use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128, Timestamp, StdResult, StdError};
use cw_storage_plus::{Item, Map};
use sha2::{Sha256, Digest};

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub access_token: Addr,
    pub rescue_delay: u64,
    pub factory: Addr,
}

/// Bit-packed timelocks structure (similar to Solidity's Timelocks type)
/// Each timelock value is stored as 8 bits (0-255), allowing 7 values in a single u64
#[cw_serde]
pub struct PackedTimelocks(pub u64);

impl PackedTimelocks {
    const MASK: u64 = 0xFF; // 8 bits mask
    const SHIFT: u64 = 8;   // 8 bits per value

    /// Create packed timelocks from individual values
    pub fn new(
        src_withdrawal: u8,
        src_public_withdrawal: u8,
        src_cancellation: u8,
        src_public_cancellation: u8,
        dst_withdrawal: u8,
        dst_public_withdrawal: u8,
        dst_cancellation: u8,
    ) -> Self {
        let mut packed = 0u64;
        packed |= src_withdrawal as u64;
        packed |= (src_public_withdrawal as u64) << Self::SHIFT;
        packed |= (src_cancellation as u64) << (Self::SHIFT * 2);
        packed |= (src_public_cancellation as u64) << (Self::SHIFT * 3);
        packed |= (dst_withdrawal as u64) << (Self::SHIFT * 4);
        packed |= (dst_public_withdrawal as u64) << (Self::SHIFT * 5);
        packed |= (dst_cancellation as u64) << (Self::SHIFT * 6);
        Self(packed)
    }

    /// Extract individual timelock values
    pub fn src_withdrawal(&self) -> u8 {
        (self.0 & Self::MASK) as u8
    }

    pub fn src_public_withdrawal(&self) -> u8 {
        ((self.0 >> Self::SHIFT) & Self::MASK) as u8
    }

    pub fn src_cancellation(&self) -> u8 {
        ((self.0 >> (Self::SHIFT * 2)) & Self::MASK) as u8
    }

    pub fn src_public_cancellation(&self) -> u8 {
        ((self.0 >> (Self::SHIFT * 3)) & Self::MASK) as u8
    }

    pub fn dst_withdrawal(&self) -> u8 {
        ((self.0 >> (Self::SHIFT * 4)) & Self::MASK) as u8
    }

    pub fn dst_public_withdrawal(&self) -> u8 {
        ((self.0 >> (Self::SHIFT * 5)) & Self::MASK) as u8
    }

    pub fn dst_cancellation(&self) -> u8 {
        ((self.0 >> (Self::SHIFT * 6)) & Self::MASK) as u8
    }

    /// Get stage time in seconds (converts from hours to seconds)
    pub fn get_stage_time(&self, stage: TimelockStage, deployed_at: u64) -> u64 {
        let hours = match stage {
            TimelockStage::SrcWithdrawal => self.src_withdrawal() as u64,
            TimelockStage::SrcPublicWithdrawal => self.src_public_withdrawal() as u64,
            TimelockStage::SrcCancellation => self.src_cancellation() as u64,
            TimelockStage::SrcPublicCancellation => self.src_public_cancellation() as u64,
            TimelockStage::DstWithdrawal => self.dst_withdrawal() as u64,
            TimelockStage::DstPublicWithdrawal => self.dst_public_withdrawal() as u64,
            TimelockStage::DstCancellation => self.dst_cancellation() as u64,
        };
        deployed_at + (hours * 3600) // Convert hours to seconds
    }

    /// Check if current time is within a specific stage
    pub fn is_within_stage(&self, current_time: u64, stage: TimelockStage, deployed_at: u64) -> bool {
        let stage_time = self.get_stage_time(stage, deployed_at);
        current_time >= stage_time
    }

    /// Calculate rescue start time
    pub fn rescue_start(&self, rescue_delay: u64, deployed_at: u64) -> u64 {
        deployed_at + rescue_delay
    }
}

/// Legacy timelocks structure (for backward compatibility)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelockStage {
    SrcWithdrawal,
    SrcPublicWithdrawal,
    SrcCancellation,
    SrcPublicCancellation,
    DstWithdrawal,
    DstPublicWithdrawal,
    DstCancellation,
}

/// Core immutables structure (matches Solidity IBaseEscrow.Immutable)
#[cw_serde]
pub struct Immutables {
    pub order_hash: String,      // bytes32 equivalent
    pub hashlock: String,        // bytes32 equivalent (hash of secret)
    pub maker: Addr,             // Address equivalent
    pub taker: Addr,             // Address equivalent
    pub token: Addr,             // Address equivalent
    pub amount: Uint128,         // uint256 equivalent
    pub safety_deposit: Uint128, // uint256 equivalent
    pub timelocks: PackedTimelocks, // Packed timelocks
    pub deployed_at: u64,        // Deployment timestamp
}

impl Immutables {
    /// Generate deterministic hash (equivalent to Solidity's keccak256)
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.order_hash.as_bytes());
        hasher.update(self.hashlock.as_bytes());
        hasher.update(self.maker.as_str().as_bytes());
        hasher.update(self.taker.as_str().as_bytes());
        hasher.update(self.token.as_str().as_bytes());
        hasher.update(self.amount.to_string().as_bytes());
        hasher.update(self.safety_deposit.to_string().as_bytes());
        hasher.update(self.timelocks.0.to_string().as_bytes());
        hasher.update(self.deployed_at.to_string().as_bytes());
        
        format!("{:x}", hasher.finalize())
    }

    /// Validate immutables structure
    pub fn validate(&self) -> StdResult<()> {
        if self.order_hash.is_empty() {
            return Err(StdError::generic_err("Order hash cannot be empty"));
        }
        if self.hashlock.is_empty() {
            return Err(StdError::generic_err("Hashlock cannot be empty"));
        }
        if self.amount == Uint128::zero() {
            return Err(StdError::generic_err("Amount cannot be zero"));
        }
        if self.safety_deposit == Uint128::zero() {
            return Err(StdError::generic_err("Safety deposit cannot be zero"));
        }
        Ok(())
    }

    /// Get stage time for a specific timelock stage
    pub fn get_stage_time(&self, stage: TimelockStage) -> u64 {
        self.timelocks.get_stage_time(stage, self.deployed_at)
    }

    /// Check if current time is within a specific stage
    pub fn is_within_stage(&self, current_time: u64, stage: TimelockStage) -> bool {
        self.timelocks.is_within_stage(current_time, stage, self.deployed_at)
    }
}

/// Cross-chain complement for destination chain
#[cw_serde]
pub struct DstImmutablesComplement {
    pub maker: Addr,
    pub amount: Uint128,
    pub token: Addr,
    pub safety_deposit: Uint128,
    pub chain_id: String,
}

/// Escrow information structure
#[cw_serde]
pub struct EscrowInfo {
    pub immutables: Immutables,
    pub dst_complement: Option<DstImmutablesComplement>,
    pub is_src: bool,
    pub is_active: bool,
    pub created_at: Timestamp,
}

/// Complete escrow state
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

/// Storage helper functions
pub fn get_next_escrow_id(storage: &mut dyn cosmwasm_std::Storage) -> StdResult<u64> {
    let current_id = ESCROW_COUNTER.load(storage).unwrap_or(0);
    let next_id = current_id + 1;
    ESCROW_COUNTER.save(storage, &next_id)?;
    Ok(next_id)
}

/// Save escrow with hash mapping
pub fn save_escrow(
    storage: &mut dyn cosmwasm_std::Storage,
    escrow_id: u64,
    escrow_state: &EscrowState,
) -> StdResult<()> {
    let escrow_hash = escrow_state.escrow_info.immutables.hash();
    
    // Save escrow by ID
    ESCROWS.save(storage, escrow_id, escrow_state)?;
    
    // Save escrow by hash for deterministic lookup
    ESCROW_BY_HASH.save(storage, escrow_hash, &escrow_id)?;
    
    Ok(())
}

/// Load escrow by ID
pub fn load_escrow(
    storage: &dyn cosmwasm_std::Storage,
    escrow_id: u64,
) -> StdResult<EscrowState> {
    ESCROWS.load(storage, escrow_id)
}

/// Load escrow by hash
pub fn load_escrow_by_hash(
    storage: &dyn cosmwasm_std::Storage,
    hash: String,
) -> StdResult<Option<u64>> {
    ESCROW_BY_HASH.may_load(storage, hash)
}

/// Check if escrow exists by hash
pub fn escrow_exists_by_hash(
    storage: &dyn cosmwasm_std::Storage,
    hash: String,
) -> bool {
    ESCROW_BY_HASH.has(storage, hash)
} 