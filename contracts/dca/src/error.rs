use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

/// ## Description
/// This enum describes DCA contract errors
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Event of zero transfer")]
    InvalidZeroAmount {},

    #[error("Provided spread amount exceeds allowed limit")]
    AllowedSpreadAssertion {},

    #[error("Operation exceeds max spread limit")]
    MaxSpreadAssertion {},

    #[error("Token has already been used to DCA")]
    AlreadyDeposited {},

    #[error("DCA amount is not equal to allowance set by token")]
    InvalidTokenDeposit {},

    #[error("Invalid hop route through {token} due to token whitelist")]
    InvalidHopRoute { token: String },

    #[error("The user does not have the specified initial_asset to dca")]
    NonexistentSwap {},

    #[error("Swap exceeds maximum of {hops} hops")]
    MaxHopsAssertion { hops: u32 },

    #[error("Tip balance is insufficient to pay performer")]
    InsufficientTipBalance {},

    #[error("The hop route specified was empty")]
    EmptyHopRoute {},

    #[error("DCA purchase occurred too early")]
    PurchaseTooEarly {},

    #[error("Hop route does not end up at target_asset")]
    TargetAssetAssertion {},
}
