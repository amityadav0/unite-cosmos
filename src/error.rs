use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    // Access Control Errors
    #[error("Unauthorized: {reason}")]
    Unauthorized { reason: String },

    #[error("Only taker can execute this function")]
    OnlyTaker {},

    #[error("Only maker can execute this function")]
    OnlyMaker {},

    #[error("Only access token holder can execute this function")]
    OnlyAccessTokenHolder {},

    #[error("Invalid caller: expected {expected}, got {actual}")]
    InvalidCaller { expected: String, actual: String },

    // Validation Errors
    #[error("Invalid immutables: {reason}")]
    InvalidImmutables { reason: String },

    #[error("Invalid secret: hash mismatch")]
    InvalidSecret {},

    #[error("Invalid escrow hash")]
    InvalidEscrowHash {},

    #[error("Invalid timelock stage: {stage}")]
    InvalidTimelockStage { stage: String },

    // Time-based Errors
    #[error("Invalid time: {reason}")]
    InvalidTime { reason: String },

    #[error("Timelock not expired: stage {stage}")]
    TimelockNotExpired { stage: String },

    #[error("Timelock expired: stage {stage}")]
    TimelockExpired { stage: String },

    #[error("Rescue delay not met: {current} < {required}")]
    RescueDelayNotMet { current: u64, required: u64 },

    #[error("Invalid creation time")]
    InvalidCreationTime {},

    // State Errors
    #[error("Escrow not found: id {escrow_id}")]
    EscrowNotFound { escrow_id: u64 },

    #[error("Escrow already exists: hash {hash}")]
    EscrowAlreadyExists { hash: String },

    #[error("Escrow not active: id {escrow_id}")]
    EscrowNotActive { escrow_id: u64 },

    #[error("Escrow already completed: id {escrow_id}")]
    EscrowAlreadyCompleted { escrow_id: u64 },

    // Balance Errors
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: String, available: String },

    #[error("Insufficient access token balance: required {required}, available {available}")]
    InsufficientAccessTokenBalance { required: String, available: String },

    // Token Transfer Errors
    #[error("Native token sending failure: {reason}")]
    NativeTokenSendingFailure { reason: String },

    #[error("CW20 token transfer failure: {reason}")]
    Cw20TokenTransferFailure { reason: String },

    #[error("Token transfer failed: {reason}")]
    TokenTransferFailed { reason: String },

    // Configuration Errors
    #[error("Access token required but not configured")]
    AccessTokenRequired {},

    #[error("Invalid token address: {address}")]
    InvalidTokenAddress { address: String },

    #[error("Invalid amount: {amount}")]
    InvalidAmount { amount: String },

    // Cross-chain Errors
    #[error("Cross-chain operation not supported: {operation}")]
    CrossChainNotSupported { operation: String },

    #[error("Invalid chain ID: {chain_id}")]
    InvalidChainId { chain_id: String },

    // Security Errors
    #[error("Security violation: {reason}")]
    SecurityViolation { reason: String },

    #[error("Reentrancy detected")]
    ReentrancyDetected {},

    #[error("Invalid signature")]
    InvalidSignature {},

    // Generic Errors
    #[error("Operation failed: {reason}")]
    OperationFailed { reason: String },

    #[error("Internal error: {reason}")]
    InternalError { reason: String },
} 