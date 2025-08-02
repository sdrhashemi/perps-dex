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
    pub fn initialize_orderbook(
        ctx: Context<InitializeOrderbook>,
        side: u8,
        capacity: u32,
    ) -> Result<()> {
        utils::initialize_orderbook(ctx, side, capacity as usize)
    }

    pub fn initialize_event_queue(ctx: Context<InitializeEventQueue>) -> Result<()> {
        utils::initialize_event_queue(ctx)
    }
    pub fn initialize_margin(ctx: Context<InitializeMargin>) -> Result<()> {
        utils::initialize_margin(ctx)
    }

    pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
        utils::deposit_collateral(ctx, amount)
    }

    pub fn withdraw_collateral(ctx: Context<WithdrawCollateral>, amount: u64) -> Result<()> {
        utils::withdraw_collateral(ctx, amount)
    }

    pub fn place_limit_order(
        ctx: Context<PlaceLimitOrder>,
        side: state::Side,
        price: u64,
        qty: u64,
    ) -> Result<()> {
        utils::place_limit_order(ctx, side, price, qty)
    }

    pub fn place_market_order(
        ctx: Context<PlaceMarketOrder>,
        qty: u64,
        side: state::Side,
        max_slippage_bps: u16,
    ) -> Result<()> {
        utils::place_market_order(ctx, qty, side, max_slippage_bps)
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
