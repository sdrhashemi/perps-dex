pub mod errors;
pub mod instructions;
pub mod orderbook;
pub mod state;
pub mod utils;
use anchor_lang::prelude::*;

declare_id!("7k59y4LUVtb9t9kYVKEkQnn7e8JW4BvowLbYsLawAoBs");

use instructions::*;

#[program]
pub mod perps_dex {
    use super::*;

    pub fn initialize_market(
        ctx: Context<InitializeMarket>,
        market_nonce: u8,
        params: state::MarketParams,
    ) -> Result<()> {
        utils::initialize_market(ctx, market_nonce, params)
    }

    pub fn place_limit_order(
        ctx: Context<PlaceLimitOrder>,
        price: u64,
        qty: u64,
        side: state::Side,
        reduce_only: bool,
    ) -> Result<()> {
        utils::place_limit_order(ctx, price, qty, side, reduce_only)
    }

    pub fn place_market_order(
        ctx: Context<PlaceMarketOrder>,
        qty: u64,
        side: state::Side,
    ) -> Result<()> {
        utils::place_market_order(ctx, qty, side)
    }

    pub fn settle_funding(ctx: Context<SettleFunding>) -> Result<()> {
        utils::settle_funding(ctx)
    }

    pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
        utils::liquidate(ctx)
    }

    pub fn update_risk_params(
        ctx: Context<UpdateRiskParams>,
        new_params: state::MarketParams,
    ) -> Result<()> {
        utils::update_risk_params(ctx, new_params)
    }
}
