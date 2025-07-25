use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid caller")]
    InvalidCaller {},

    #[error("Invalid immutables")]
    InvalidImmutables {},

    #[error("Invalid secret")]
    InvalidSecret {},

    #[error("Invalid time")]
    InvalidTime {},

    #[error("Escrow not found")]
    EscrowNotFound {},

    #[error("Escrow already exists")]
    EscrowAlreadyExists {},

    #[error("Insufficient balance")]
    InsufficientBalance {},

    #[error("Escrow not active")]
    EscrowNotActive {},

    #[error("Invalid timelock stage")]
    InvalidTimelockStage {},

    #[error("Native token sending failure")]
    NativeTokenSendingFailure {},

    #[error("Access token required")]
    AccessTokenRequired {},

    #[error("Rescue delay not met")]
    RescueDelayNotMet {},

    #[error("Invalid escrow hash")]
    InvalidEscrowHash {},
} 