use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized action")]
    Unauthorized,
    #[msg("Market not initialized")]
    MarketUninitialized,
    #[msg("Insufficient collateral")]
    InsufficientCollateral,
    #[msg("Orderbook overflow")]
    OrderbookOverflow,
    #[msg("Price deviation too large")]
    PriceDeviation,
    #[msg("Invalid price for pyth")]
    InvalidPriceFeed,
    #[msg("Stale prices from ocale")]
    StalePrice,
    #[msg("Invalid healthy account")]
    HealthyAccount,
    #[msg("Event serialization failed")]
    EventSerializationFailure,
    #[msg("Overflow hit")]
    Overflow,
    #[msg("Invalid amount for deposit or withdraw")]
    InvalidAmount,
    #[msg("Failed to deserialize event")]
    EventDeserializationFailure,
    #[msg("Proposal has already been executed")]
    ProposalAlreadyExecuted,
    #[msg("Proposal has not passed")]
    ProposalNotPassed,
}
