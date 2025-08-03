use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Transfer};

use crate::{errors::ErrorCode, state::{MarginAccount, Market}};

#[derive(Accounts)]
pub struct DepositCollateral<'info> {
    #[account(mut, has_one = authority)]
    pub market: Account<'info, Market>,
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"margin", market.key().as_ref(), user.key().as_ref()], bump)]
    pub margin: Account<'info, MarginAccount>,
    pub user: Signer<'info>,
    #[account(mut, constraint = user_collateral.owner == user.key())]
    pub user_collateral: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut, constraint = market_vault.owner == market.key())]
    pub market_vault: Account<'info, anchor_spl::token::TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawCollateral<'info> {
    #[account(mut, has_one = authority)]
    pub market: Account<'info, Market>,
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"margin", market.key().as_ref(), user.key().as_ref()], bump)]
    pub margin: Account<'info, MarginAccount>,
    pub user: Signer<'info>,
    #[account(mut, constraint = market_vault.owner == market.key())]
    pub market_vault: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut, constraint = user_collateral.owner == user.key())]
    pub user_collateral: Account<'info, anchor_spl::token::TokenAccount>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let cpi_accounts = Transfer {
        from: ctx.accounts.user_collateral.to_account_info(),
        to: ctx.accounts.market_vault.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, amount)?;

    let margin = &mut ctx.accounts.margin;
    margin.collateral = margin.collateral.saturating_add(amount);

    Ok(())
}

pub fn withdraw_collateral(ctx: Context<WithdrawCollateral>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let margin = &mut ctx.accounts.margin;
    let after = margin
        .collateral
        .checked_sub(amount)
        .ok_or(error!(ErrorCode::InsufficientCollateral))?;

    margin.collateral = after;

    let cpi_accounts = Transfer {
        from: ctx.accounts.market_vault.to_account_info(),
        to: ctx.accounts.user_collateral.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, amount)?;

    Ok(())
}
