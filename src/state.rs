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

/// Escrow type to differentiate source vs destination behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum EscrowType {
    Source,     // EscrowSrc behavior
    Destination, // EscrowDst behavior
}

impl EscrowType {
    /// Check if this is a source escrow
    pub fn is_source(&self) -> bool {
        matches!(self, EscrowType::Source)
    }

    /// Check if this is a destination escrow
    pub fn is_destination(&self) -> bool {
        matches!(self, EscrowType::Destination)
    }

    /// Get the appropriate withdrawal recipient based on escrow type
    pub fn get_withdrawal_recipient(&self, maker: &Addr, taker: &Addr) -> Addr {
        match self {
            EscrowType::Source => taker.clone(),
            EscrowType::Destination => maker.clone(),
        }
    }

    /// Get the appropriate cancellation recipient based on escrow type
    pub fn get_cancellation_recipient(&self, maker: &Addr, taker: &Addr) -> Addr {
        match self {
            EscrowType::Source => maker.clone(),
            EscrowType::Destination => taker.clone(),
        }
    }

    /// Get the appropriate withdrawal stage based on escrow type
    pub fn get_withdrawal_stage(&self) -> TimelockStage {
        match self {
            EscrowType::Source => TimelockStage::SrcWithdrawal,
            EscrowType::Destination => TimelockStage::DstWithdrawal,
        }
    }

    /// Get the appropriate cancellation stage based on escrow type
    pub fn get_cancellation_stage(&self) -> TimelockStage {
        match self {
            EscrowType::Source => TimelockStage::SrcCancellation,
            EscrowType::Destination => TimelockStage::DstCancellation,
        }
    }

    /// Get the appropriate public withdrawal stage based on escrow type
    pub fn get_public_withdrawal_stage(&self) -> TimelockStage {
        match self {
            EscrowType::Source => TimelockStage::SrcPublicWithdrawal,
            EscrowType::Destination => TimelockStage::DstPublicWithdrawal,
        }
    }

    /// Get the appropriate public cancellation stage based on escrow type
    pub fn get_public_cancellation_stage(&self) -> Option<TimelockStage> {
        match self {
            EscrowType::Source => Some(TimelockStage::SrcPublicCancellation),
            EscrowType::Destination => None, // Destination has no public cancellation
        }
    }

    /// Check if this escrow type supports public cancellation
    pub fn supports_public_cancellation(&self) -> bool {
        self.get_public_cancellation_stage().is_some()
    }
}

/// Timelock stages matching Solidity enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelockStage {
    SrcWithdrawal,           // 0: Source private withdrawal
    SrcPublicWithdrawal,     // 1: Source public withdrawal
    SrcCancellation,         // 2: Source private cancellation
    SrcPublicCancellation,   // 3: Source public cancellation
    DstWithdrawal,           // 4: Destination private withdrawal
    DstPublicWithdrawal,     // 5: Destination public withdrawal
    DstCancellation,         // 6: Destination private cancellation
}

impl TimelockStage {
    /// Convert stage to bit offset for packing
    pub fn bit_offset(&self) -> u64 {
        match self {
            TimelockStage::SrcWithdrawal => 0,
            TimelockStage::SrcPublicWithdrawal => 1,
            TimelockStage::SrcCancellation => 2,
            TimelockStage::SrcPublicCancellation => 3,
            TimelockStage::DstWithdrawal => 4,
            TimelockStage::DstPublicWithdrawal => 5,
            TimelockStage::DstCancellation => 6,
        }
    }

    /// Check if stage is for source chain
    pub fn is_source(&self) -> bool {
        matches!(self, 
            TimelockStage::SrcWithdrawal | 
            TimelockStage::SrcPublicWithdrawal | 
            TimelockStage::SrcCancellation | 
            TimelockStage::SrcPublicCancellation
        )
    }

    /// Check if stage is for destination chain
    pub fn is_destination(&self) -> bool {
        matches!(self, 
            TimelockStage::DstWithdrawal | 
            TimelockStage::DstPublicWithdrawal | 
            TimelockStage::DstCancellation
        )
    }

    /// Check if stage is public (anyone can call)
    pub fn is_public(&self) -> bool {
        matches!(self, 
            TimelockStage::SrcPublicWithdrawal | 
            TimelockStage::SrcPublicCancellation | 
            TimelockStage::DstPublicWithdrawal
        )
    }

    /// Check if stage is private (only specific parties can call)
    pub fn is_private(&self) -> bool {
        !self.is_public()
    }

    /// Get the escrow type for this stage
    pub fn get_escrow_type(&self) -> EscrowType {
        if self.is_source() {
            EscrowType::Source
        } else {
            EscrowType::Destination
        }
    }
}

/// Sophisticated bit-packed timelocks structure
/// Matches Solidity TimelocksLib.sol implementation
/// 
/// Bit layout (64 bits total):
/// - Bits 0-31: deployed_at timestamp (32 bits)
/// - Bits 32-39: src_withdrawal (8 bits, 0-255 hours)
/// - Bits 40-47: src_public_withdrawal (8 bits, 0-255 hours)
/// - Bits 48-55: src_cancellation (8 bits, 0-255 hours)
/// - Bits 56-63: src_public_cancellation (8 bits, 0-255 hours)
/// - Additional 64 bits for destination timelocks
#[cw_serde]
pub struct PackedTimelocks {
    /// Source chain timelocks + deployed_at (64 bits)
    pub source_data: u64,
    /// Destination chain timelocks (64 bits)
    pub destination_data: u64,
}

impl PackedTimelocks {
    // Bit masks and offsets
    const DEPLOYED_AT_MASK: u64 = 0xFFFFFFFF; // 32 bits
    const TIMELOCK_MASK: u64 = 0xFF; // 8 bits
    const TIMELOCK_SHIFT: u64 = 8;
    const DEPLOYED_AT_OFFSET: u64 = 32;

    /// Create packed timelocks from individual values
    pub fn new(
        deployed_at: u32,
        src_withdrawal: u8,
        src_public_withdrawal: u8,
        src_cancellation: u8,
        src_public_cancellation: u8,
        dst_withdrawal: u8,
        dst_public_withdrawal: u8,
        dst_cancellation: u8,
    ) -> Self {
        // Pack source data: deployed_at (32 bits) + 4 timelocks (8 bits each)
        let mut source_data = deployed_at as u64;
        source_data |= (src_withdrawal as u64) << Self::DEPLOYED_AT_OFFSET;
        source_data |= (src_public_withdrawal as u64) << (Self::DEPLOYED_AT_OFFSET + Self::TIMELOCK_SHIFT);
        source_data |= (src_cancellation as u64) << (Self::DEPLOYED_AT_OFFSET + Self::TIMELOCK_SHIFT * 2);
        source_data |= (src_public_cancellation as u64) << (Self::DEPLOYED_AT_OFFSET + Self::TIMELOCK_SHIFT * 3);

        // Pack destination data: 3 timelocks (8 bits each)
        let mut destination_data = 0u64;
        destination_data |= dst_withdrawal as u64;
        destination_data |= (dst_public_withdrawal as u64) << Self::TIMELOCK_SHIFT;
        destination_data |= (dst_cancellation as u64) << (Self::TIMELOCK_SHIFT * 2);

        Self {
            source_data,
            destination_data,
        }
    }

    /// Get deployed_at timestamp
    pub fn deployed_at(&self) -> u32 {
        (self.source_data & Self::DEPLOYED_AT_MASK) as u32
    }

    /// Get timelock value for a specific stage (matches Solidity get() function)
    pub fn get(&self, stage: TimelockStage) -> u8 {
        match stage {
            TimelockStage::SrcWithdrawal => {
                ((self.source_data >> Self::DEPLOYED_AT_OFFSET) & Self::TIMELOCK_MASK) as u8
            }
            TimelockStage::SrcPublicWithdrawal => {
                ((self.source_data >> (Self::DEPLOYED_AT_OFFSET + Self::TIMELOCK_SHIFT)) & Self::TIMELOCK_MASK) as u8
            }
            TimelockStage::SrcCancellation => {
                ((self.source_data >> (Self::DEPLOYED_AT_OFFSET + Self::TIMELOCK_SHIFT * 2)) & Self::TIMELOCK_MASK) as u8
            }
            TimelockStage::SrcPublicCancellation => {
                ((self.source_data >> (Self::DEPLOYED_AT_OFFSET + Self::TIMELOCK_SHIFT * 3)) & Self::TIMELOCK_MASK) as u8
            }
            TimelockStage::DstWithdrawal => {
                (self.destination_data & Self::TIMELOCK_MASK) as u8
            }
            TimelockStage::DstPublicWithdrawal => {
                ((self.destination_data >> Self::TIMELOCK_SHIFT) & Self::TIMELOCK_MASK) as u8
            }
            TimelockStage::DstCancellation => {
                ((self.destination_data >> (Self::TIMELOCK_SHIFT * 2)) & Self::TIMELOCK_MASK) as u8
            }
        }
    }

    /// Get stage time in seconds (converts hours to seconds)
    pub fn get_stage_time(&self, stage: TimelockStage) -> u64 {
        let hours = self.get(stage) as u64;
        let deployed_at = self.deployed_at() as u64;
        deployed_at + (hours * 3600) // Convert hours to seconds
    }

    /// Check if current time is within a specific stage
    pub fn is_within_stage(&self, current_time: u64, stage: TimelockStage) -> bool {
        let stage_time = self.get_stage_time(stage);
        current_time >= stage_time
    }

    /// Check if a stage has passed (current time > stage time)
    pub fn has_stage_passed(&self, current_time: u64, stage: TimelockStage) -> bool {
        let stage_time = self.get_stage_time(stage);
        current_time > stage_time
    }

    /// Get the next valid stage based on current time
    pub fn get_current_stage(&self, current_time: u64) -> Option<TimelockStage> {
        let stages = [
            TimelockStage::SrcWithdrawal,
            TimelockStage::SrcPublicWithdrawal,
            TimelockStage::SrcCancellation,
            TimelockStage::SrcPublicCancellation,
            TimelockStage::DstWithdrawal,
            TimelockStage::DstPublicWithdrawal,
            TimelockStage::DstCancellation,
        ];

        stages.into_iter().find(|&stage| self.is_within_stage(current_time, stage))
    }

    /// Calculate rescue start time
    pub fn rescue_start(&self, rescue_delay: u64) -> u64 {
        let deployed_at = self.deployed_at() as u64;
        deployed_at + rescue_delay
    }

    /// Check if rescue is available (current time >= rescue start)
    pub fn is_rescue_available(&self, current_time: u64, rescue_delay: u64) -> bool {
        let rescue_start = self.rescue_start(rescue_delay);
        current_time >= rescue_start
    }

    /// Validate timelock values (ensure logical progression)
    pub fn validate(&self) -> StdResult<()> {
        let deployed_at = self.deployed_at();
        if deployed_at == 0 {
            return Err(StdError::generic_err("Deployed timestamp cannot be zero"));
        }

        // Validate source chain progression
        let src_withdrawal = self.get(TimelockStage::SrcWithdrawal);
        let src_public_withdrawal = self.get(TimelockStage::SrcPublicWithdrawal);
        let src_cancellation = self.get(TimelockStage::SrcCancellation);
        let src_public_cancellation = self.get(TimelockStage::SrcPublicCancellation);

        if src_public_withdrawal <= src_withdrawal {
            return Err(StdError::generic_err("Source public withdrawal must be after private withdrawal"));
        }
        if src_cancellation <= src_public_withdrawal {
            return Err(StdError::generic_err("Source cancellation must be after public withdrawal"));
        }
        if src_public_cancellation <= src_cancellation {
            return Err(StdError::generic_err("Source public cancellation must be after private cancellation"));
        }

        // Validate destination chain progression
        let dst_withdrawal = self.get(TimelockStage::DstWithdrawal);
        let dst_public_withdrawal = self.get(TimelockStage::DstPublicWithdrawal);
        let dst_cancellation = self.get(TimelockStage::DstCancellation);

        if dst_public_withdrawal <= dst_withdrawal {
            return Err(StdError::generic_err("Destination public withdrawal must be after private withdrawal"));
        }
        if dst_cancellation <= dst_public_withdrawal {
            return Err(StdError::generic_err("Destination cancellation must be after public withdrawal"));
        }

        Ok(())
    }

    /// Get all timelock values as a human-readable format
    pub fn debug_info(&self) -> String {
        format!(
            "Deployed: {}, Src: [{}h, {}h, {}h, {}h], Dst: [{}h, {}h, {}h]",
            self.deployed_at(),
            self.get(TimelockStage::SrcWithdrawal),
            self.get(TimelockStage::SrcPublicWithdrawal),
            self.get(TimelockStage::SrcCancellation),
            self.get(TimelockStage::SrcPublicCancellation),
            self.get(TimelockStage::DstWithdrawal),
            self.get(TimelockStage::DstPublicWithdrawal),
            self.get(TimelockStage::DstCancellation),
        )
    }
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
        hasher.update(self.timelocks.source_data.to_string().as_bytes());
        hasher.update(self.timelocks.destination_data.to_string().as_bytes());
        
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
        
        // Validate timelocks
        self.timelocks.validate()?;
        
        Ok(())
    }

    /// Get stage time for a specific timelock stage
    pub fn get_stage_time(&self, stage: TimelockStage) -> u64 {
        self.timelocks.get_stage_time(stage)
    }

    /// Check if current time is within a specific stage
    pub fn is_within_stage(&self, current_time: u64, stage: TimelockStage) -> bool {
        self.timelocks.is_within_stage(current_time, stage)
    }

    /// Check if rescue is available
    pub fn is_rescue_available(&self, current_time: u64, rescue_delay: u64) -> bool {
        self.timelocks.is_rescue_available(current_time, rescue_delay)
    }

    /// Get current stage based on time
    pub fn get_current_stage(&self, current_time: u64) -> Option<TimelockStage> {
        self.timelocks.get_current_stage(current_time)
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
    pub escrow_type: EscrowType, // Source or Destination
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

/// Storage helper functions
pub fn get_next_escrow_id(storage: &mut dyn cosmwasm_std::Storage) -> StdResult<u64> {
    let current_id = ESCROW_COUNTER.load(storage).unwrap_or(0);
    let next_id = current_id + 1;
    ESCROW_COUNTER.save(storage, &next_id)?;
    Ok(next_id)
}

/// Load escrow by ID
pub fn load_escrow(
    storage: &dyn cosmwasm_std::Storage,
    escrow_id: u64,
) -> StdResult<EscrowState> {
    ESCROWS.load(storage, escrow_id)
} 