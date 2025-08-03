use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

use crate::errors::ErrorCode;
use crate::slab::Slab;
use crate::state::{MarginAccount, Market, OrderbookSide, Side};
use crate::utils::get_mark_price;

#[derive(Accounts)]
pub struct LiquidateEngine<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub margin: Account<'info, MarginAccount>,
    #[account(mut)]
    pub orderbook_side: Account<'info, OrderbookSide>,
    #[account(
        mut,
        seeds = [b"slab", orderbook_side.key().as_ref()],
        bump = orderbook_side.bump
    )]
    pub slab: AccountLoader<'info, Slab>,
    /// CHECK:
    #[account(mut)]
    pub oracle_pyth: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    pub oracle_switch: AccountInfo<'info>,
    pub liquidator: Signer<'info>,
    #[account(mut)]
    pub liquidator_collateral_account: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut)]
    pub collateral_vault: Account<'info, anchor_spl::token::TokenAccount>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn liquidate(ctx: Context<LiquidateEngine>) -> Result<()> {
    // fetch resilient mark price
    let mut pyth_ai = ctx.accounts.oracle_pyth.clone();
    let mut sb_ai = ctx.accounts.oracle_switch.clone();
    let params = &ctx.accounts.market.params;
    let max_age = ctx.accounts.market.params.funding_interval;
    let mark_price = get_mark_price(&mut pyth_ai, &mut sb_ai, max_age, 5, 3)?;

    // compute equity & notional
    let margin = &mut ctx.accounts.margin;
    let mut equity: i128 = margin.collateral as i128;
    let mut notional: i128 = 0;
    for pos in margin.positions.iter() {
        let entry = pos.entry_price as i128;
        let sign = if pos.side == Side::Bid { 1 } else { -1 };
        let pnl = (mark_price - entry).saturating_mul(pos.qty as i128) * sign;
        equity = equity.saturating_add(pnl);
        notional = notional.saturating_add((entry.saturating_mul(pos.qty as i128)).abs());
    }

    // maintenance margin check
    let health = if notional > 0 {
        equity.saturating_mul(10_000) / notional
    } else {
        0
    };
    require!(
        health < params.maintenance_margin_ratio as i128,
        ErrorCode::HealthyAccount
    );

    // unwind positions via in-place slab
    let mut slab = ctx.accounts.slab.load_mut()?;
    let mut total_proceeds: u128 = 0;
    for pos in margin.positions.iter() {
        let mut rem = pos.qty;
        while rem > 0 {
            if let Some(idx) = slab.best() {
                // temporarily mutate node and capture data, then drop borrow
                let (price, trade_qty, emptied) = {
                    let node = &mut slab.nodes[idx as usize];
                    let tq = rem.min(node.qty);
                    node.qty -= tq;
                    (node.price, tq, node.qty == 0)
                };
                if emptied {
                    slab.remove(idx)?;
                }
                total_proceeds = total_proceeds
                    .saturating_add((trade_qty as u128).saturating_mul(price as u128));
                rem = rem.saturating_sub(trade_qty);
            } else {
                break;
            }
        }
    }

    // persist updated slab pointers
    let ob = &mut ctx.accounts.orderbook_side;
    ob.head = slab.head;
    ob.free_head = slab.free_head;

    // apply 0.5% liquidation fee
    let proceeds_u64: u64 = total_proceeds
        .try_into()
        .map_err(|_| error!(ErrorCode::Overflow))?;
    let fee = proceeds_u64 / 200;
    let liquidator_cut = fee;
    let vault_cut = proceeds_u64.saturating_sub(fee);

    // transfer liquidator share
    let cpi_accounts = Transfer {
        from: ctx.accounts.collateral_vault.to_account_info(),
        to: ctx.accounts.liquidator_collateral_account.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, liquidator_cut)?;

    // clear positions and set collateral
    margin.positions.clear();
    margin.collateral = vault_cut;

    Ok(())
}
