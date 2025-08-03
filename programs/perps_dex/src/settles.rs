use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

use crate::errors::ErrorCode;
use crate::state::{OrderEvent, Position};
use crate::{
    state::{EventQueue, MarginAccount, MarginType, Market, OrderbookSide, Side},
    utils::get_mark_price,
};

#[derive(Accounts)]
pub struct SettleFills<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub event_queue: Account<'info, EventQueue>,
    #[account(mut, constraint = market_vault.owner == market.key())]
    pub market_vault: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut, seeds = [b"margin", market.key().as_ref(), maker.key().as_ref()], bump)]
    pub maker_margin: Account<'info, MarginAccount>,
    #[account(mut, constraint = maker_collateral.owner == maker_margin.owner)]
    pub maker_collateral: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut, seeds = [b"margin", market.key().as_ref(), taker.key().as_ref()], bump)]
    pub taker_margin: Account<'info, MarginAccount>,
    #[account(mut, constraint = taker_collateral.owner == taker_margin.owner)]
    pub taker_collateral: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut)]
    pub orderbook_side: Account<'info, OrderbookSide>,

    pub maker: Signer<'info>,
    pub taker: Signer<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

#[derive(Accounts)]
pub struct SettleFunding<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub margin: Account<'info, MarginAccount>,
    /// CHECK:
    #[account(mut)]
    pub oracle_pyth: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    pub oracle_switchboard: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn settle_funding(ctx: Context<SettleFunding>) -> Result<()> {
    let now = ctx.accounts.clock.unix_timestamp;
    let mut pyth_ai = ctx.accounts.oracle_pyth.to_account_info();
    let mut sb_ai = ctx.accounts.oracle_switchboard.to_account_info();
    let max_age = ctx.accounts.market.params.funding_interval;
    let mark_price = get_mark_price(&mut pyth_ai, &mut sb_ai, max_age, 5, 3)?;

    // branch by margin mode
    let m = &mut ctx.accounts.margin;
    match m.margin_type {
        MarginType::Cross => {
            let mut net: i128 = 0;
            for pos in &m.positions {
                let e = pos.entry_price as i128;
                let diff = mark_price.saturating_sub(e);
                let fund = diff
                    .saturating_mul(pos.qty as i128)
                    .checked_div(e)
                    .unwrap_or(0);
                net = match pos.side {
                    Side::Bid => net.saturating_sub(fund),
                    Side::Ask => net.saturating_add(fund),
                };
            }
            if net < 0 {
                let d = (-net) as u64;
                require!(m.collateral >= d, ErrorCode::InsufficientCollateral);
                m.collateral = m.collateral.saturating_sub(d);
            } else {
                m.collateral = m.collateral.saturating_add(net as u64);
            }
        }
        MarginType::Isolated => {
            for pos in &mut m.positions {
                let e = pos.entry_price as i128;
                let diff = mark_price.saturating_sub(e);
                let fund = diff
                    .saturating_mul(pos.qty as i128)
                    .checked_div(e)
                    .unwrap_or(0);
                let delta = if pos.side == Side::Bid {
                    (fund as i128).saturating_neg() as i64 as u64
                } else {
                    fund as u64
                };
                require!(pos.collateral >= delta, ErrorCode::InsufficientCollateral);
                pos.collateral = pos.collateral.saturating_add(if pos.side == Side::Ask {
                    delta
                } else {
                    delta.wrapping_neg()
                });
            }
        }
    }

    // 4) update timestamp
    ctx.accounts.market.last_funding_timestamp = now;
    Ok(())
}

pub fn settle_fills(ctx: Context<SettleFills>) -> Result<()> {
    const ORDER_EVENT_SIZE: usize = 65;
    let queue = &mut ctx.accounts.event_queue;
    while queue.head != queue.tail {
        let idx = queue.head as usize;
        let start = idx
            .checked_mul(ORDER_EVENT_SIZE)
            .ok_or(error!(ErrorCode::Overflow))?;
        let end = start
            .checked_add(ORDER_EVENT_SIZE)
            .ok_or(error!(ErrorCode::Overflow))?;
        require!(
            end <= queue.events.len(),
            ErrorCode::EventDeserializationFailure
        );
        let data = &queue.events[start..end];
        let ev = OrderEvent::try_from_slice(data)
            .map_err(|_| error!(ErrorCode::EventDeserializationFailure))?;
        if ev.event_type == 1 {
            let amt_u128 = (ev.price as u128)
                .checked_mul(ev.qty as u128)
                .ok_or(error!(ErrorCode::Overflow))?;
            let amt: u64 = amt_u128
                .try_into()
                .map_err(|_| error!(ErrorCode::Overflow))?;
            let cpi_accounts = Transfer {
                from: ctx.accounts.taker_collateral.to_account_info(),
                to: ctx.accounts.maker_collateral.to_account_info(),
                authority: ctx.accounts.taker.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
            token::transfer(cpi_ctx, amt)?;
            let maker_margin = &mut ctx.accounts.maker_margin;
            if let Some(pos) = maker_margin.positions.iter_mut().find(|p| p.key == ev.key) {
                pos.qty = pos.qty.saturating_sub(ev.qty);
            }
            let taker_margin = &mut ctx.accounts.taker_margin;
            let taker_side = if ctx.accounts.orderbook_side.side == Side::Bid {
                Side::Ask
            } else {
                Side::Bid
            };
            if let Some(pos) = taker_margin.positions.iter_mut().find(|p| p.key == ev.key) {
                pos.qty = pos.qty.saturating_add(ev.qty);
            } else {
                taker_margin.positions.push(Position {
                    key: ev.key,
                    qty: ev.qty,
                    entry_price: ev.price,
                    side: taker_side,
                    collateral: 0,
                });
            }
        }
        queue.head = queue.head.wrapping_add(1);
    }

    ctx.accounts.maker_margin.positions.retain(|p| p.qty > 0);
    ctx.accounts.taker_margin.positions.retain(|p| p.qty > 0);
    Ok(())
}
