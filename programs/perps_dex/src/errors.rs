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
}
