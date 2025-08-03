use crate::errors::ErrorCode;
use crate::slab::Slab;
use crate::state::{EventQueue, MarginAccount, Market, OrderbookSide, Side};
use crate::utils::push_event;
use anchor_lang::prelude::*;
use anchor_lang::AnchorDeserialize;

#[derive(Accounts)]
#[instruction(side: Side)]
pub struct PlaceLimitOrder<'info> {
    /// Metadata for this side
    #[account(
        mut,
        seeds = [b"orderbook", market.key().as_ref(), &[side as u8]],
        bump = orderbook_side.bump
    )]
    pub orderbook_side: Account<'info, OrderbookSide>,

    /// Zero-copy slab buffer
    #[account(
        mut,
        seeds = [b"slab", orderbook_side.key().as_ref()],
        bump
    )]
    pub slab: AccountLoader<'info, Slab>,

    /// Event queue for orderbook events
    #[account(
        mut,
        seeds = [b"eventqueue", market.key().as_ref()],
        bump = event_queue.bump
    )]
    pub event_queue: Account<'info, EventQueue>,

    #[account(mut)]
    pub margin: Account<'info, MarginAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub market: Account<'info, Market>,

    pub token_program: Program<'info, anchor_spl::token::Token>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(side: Side)]
pub struct PlaceMarketOrder<'info> {
    #[account(
        mut,
        seeds = [b"orderbook", market.key().as_ref(), &[side as u8]],
        bump = orderbook_side.bump
    )]
    pub orderbook_side: Account<'info, OrderbookSide>,

    #[account(
        mut,
        seeds = [b"slab", orderbook_side.key().as_ref()],
        bump
    )]
    pub slab: AccountLoader<'info, Slab>,

    #[account(
        mut,
        seeds = [b"eventqueue", market.key().as_ref()],
        bump = event_queue.bump
    )]
    pub event_queue: Account<'info, EventQueue>,

    #[account(mut)]
    pub margin: Account<'info, MarginAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub market: Account<'info, Market>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}
pub fn place_limit_order(
    ctx: Context<PlaceLimitOrder>,
    side: Side,
    price: u64,
    qty: u64,
) -> Result<()> {
    let ob = &mut ctx.accounts.orderbook_side;

    msg!(
        "Starting place_limit_order: side={:?}, price={}, qty={}",
        side,
        price,
        qty
    );
    require!(ob.side == side, ErrorCode::InvalidOrderbookSide);

    let market = &ctx.accounts.market;
    let margin = &ctx.accounts.margin;
    let clock = Clock::get()?;

    let collateral = margin.collateral as u128;
    let lev_limit = market.params.leverage_limit as u128;
    let order_notional = (price as u128)
        .checked_mul(qty as u128)
        .ok_or(error!(ErrorCode::Overflow))?;

    let existing_notional: u128 = margin.positions.iter().try_fold(0u128, |acc, p| {
        (p.entry_price as u128)
            .checked_mul(p.qty as u128)
            .and_then(|notional| acc.checked_add(notional))
            .ok_or(error!(ErrorCode::Overflow))
    })?;

    let total_notional = existing_notional
        .checked_add(order_notional)
        .ok_or(error!(ErrorCode::Overflow))?;

    require!(
        total_notional <= collateral.saturating_mul(lev_limit),
        ErrorCode::LeverageExceeded
    );

    let mut slab = ctx.accounts.slab.load_mut()?;
    let key = ob.next_order_id as u128;
    slab.insert(key, price, qty, ctx.accounts.user.key(), clock.slot)?;

    ob.next_order_id = ob
        .next_order_id
        .checked_add(1)
        .ok_or(error!(ErrorCode::OrderbookOverflow))?;
    ob.head = slab.head;
    ob.free_head = slab.free_head;

    msg!(
        "Placed limit order: key={}, price={}, qty={}",
        key,
        price,
        qty
    );

    push_event(
        &mut ctx.accounts.event_queue,
        0,
        key,
        price,
        qty,
        ctx.accounts.user.key(),
    )?;

    Ok(())
}

pub fn place_market_order(
    ctx: Context<PlaceMarketOrder>,
    qty: u64,
    side: Side,
    max_slippage_bps: u16,
) -> Result<()> {
    let ob = &mut ctx.accounts.orderbook_side;
    require!(ob.side == side, ErrorCode::InvalidOrderbookSide);
    let slab_ro = ctx.accounts.slab.load()?;
    let best_idx = slab_ro.best().ok_or(error!(ErrorCode::OrderbookEmpty))?;
    let best_price = slab_ro.nodes[best_idx as usize].price;
    let allowed = best_price
        .checked_mul(10_000u64.saturating_add(max_slippage_bps as u64))
        .ok_or(error!(ErrorCode::Overflow))?
        .checked_div(10_000)
        .ok_or(error!(ErrorCode::Overflow))?;

    let mut slab = ctx.accounts.slab.load_mut()?;
    let mut remaining = qty;
    while remaining > 0 {
        if let Some(idx) = slab.best() {
            // extract fields to avoid borrow conflicts
            let (key_node, price_node, qty0, owner_node) = {
                let node_ref = &slab.nodes[idx as usize];
                (node_ref.key, node_ref.price, node_ref.qty, node_ref.owner)
            };
            require!(price_node <= allowed, ErrorCode::SlippageExceeded);
            let trade_qty = remaining.min(qty0);
            if trade_qty == qty0 {
                slab.remove(idx)?;
            } else {
                slab.nodes[idx as usize].qty = qty0 - trade_qty;
            }
            push_event(
                &mut ctx.accounts.event_queue,
                1,
                key_node,
                price_node,
                trade_qty,
                owner_node,
            )?;
            remaining = remaining.saturating_sub(trade_qty);
        } else {
            break;
        }
    }

    ob.head = slab.head;
    ob.free_head = slab.free_head;
    Ok(())
}
