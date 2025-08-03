use anchor_lang::prelude::*;

use crate::state::{Market, MarketParams};

#[derive(Accounts)]
#[instruction(market_nonce: u8, params: crate::state::MarketParams)]
pub struct InitializeMarket<'info> {
    #[account(
        init,
        seeds = [b"market", base_mint.key().as_ref(), quote_mint.key().as_ref(), &[market_nonce]],
        bump,
        payer = authority,
        space = 8 + std::mem::size_of::<Market>()
    )]
    pub market: Account<'info, Market>,
    /// CHECK: This is a token mint account
    pub base_mint: AccountInfo<'info>,
    /// CHECK: This is a token mint account
    pub quote_mint: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

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
