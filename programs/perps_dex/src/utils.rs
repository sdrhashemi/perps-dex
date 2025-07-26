use crate::errors::ErrorCode;
use crate::instructions::{
    InitializeMarket, Liquidate, PlaceLimitOrder, PlaceMarketOrder, SettleFunding, UpdateRiskParams,
};
use crate::orderbook::Slab;
use crate::state::{EventQueue, MarginAccount, Market, MarketParams, OrderbookSide, Side};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

/// Decode a Slab from raw bytes storage (stub: alloc fresh slab)
fn decode_slab(data: &[u8]) -> Result<Slab> {
    let capacity = data.len() / std::mem::size_of::<crate::orderbook::SlabNode>();
    Ok(Slab::new(capacity))
}

/// Encode a Slab back into raw bytes storage (stub)
fn encode_slab(_slab: &Slab) -> Vec<u8> {
    Vec::new()
}

/// Initialize the market account
pub fn initialize_market(
    ctx: Context<InitializeMarket>,
    market_nonce: u8,
    params: MarketParams,
) -> Result<()> {
    let m = &mut ctx.accounts.market;
    m.authority = *ctx.accounts.authority.key;
    m.base_mint = ctx.accounts.base_mint.key();
    m.quote_mint = ctx.accounts.quote_mint.key();
    m.oracle_pyth = Pubkey::default();
    m.oracle_switchboard = Pubkey::default();
    m.params = params;
    m.nonce = market_nonce;
    Ok(())
}

/// Place a limit (maker) order
pub fn place_limit_order(
    ctx: Context<PlaceLimitOrder>,
    price: u64,
    qty: u64,
    side: Side,
    _reduce_only: bool,
) -> Result<()> {
    let ob = &mut ctx.accounts.orderbook_side;
    let mut slab = decode_slab(&ob.slab)?;
    let key = ob.next_order_id as u128;
    slab.insert(key, price, qty, ctx.accounts.user.key(), side)?;
    ob.next_order_id = ob
        .next_order_id
        .checked_add(1)
        .ok_or(ErrorCode::OrderbookOverflow)?;
    ob.slab = encode_slab(&slab);
    Ok(())
}

/// Place a market (taker) order
pub fn place_market_order(ctx: Context<PlaceMarketOrder>, qty: u64, side: Side) -> Result<()> {
    let ob = &mut ctx.accounts.orderbook_side;
    let mut slab = decode_slab(&ob.slab)?;
    let mut remaining = qty;
    while remaining > 0 {
        if let Some(best_idx) = slab.find_best() {
            let node = &slab.nodes[best_idx as usize];
            let trade_qty = remaining.min(node.qty);
            slab.reduce_order(best_idx, trade_qty)?;
            remaining = remaining.saturating_sub(trade_qty);
        } else {
            break;
        }
    }
    ob.slab = encode_slab(&slab);
    Ok(())
}

/// Settle funding fees
pub fn settle_funding(ctx: Context<SettleFunding>) -> Result<()> {
    let margin = &mut ctx.accounts.margin;
    let fee: u64 = 1;
    require!(margin.collateral >= fee, ErrorCode::InsufficientCollateral);
    margin.collateral = margin.collateral.saturating_sub(fee);
    Ok(())
}

/// Liquidate an under-collateralized account
pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
    let margin = &mut ctx.accounts.margin;
    let penalty = margin.collateral / 10;
    margin.collateral = margin.collateral.saturating_sub(penalty);
    margin.positions.clear();
    Ok(())
}

/// Update risk parameters via DAO authority
pub fn update_risk_params(ctx: Context<UpdateRiskParams>, new_params: MarketParams) -> Result<()> {
    let m = &mut ctx.accounts.market;
    m.params = new_params;
    Ok(())
}
