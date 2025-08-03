use anchor_lang::prelude::*;

use crate::state::{MarginAccount, MarginType, Market};

#[derive(Accounts)]
#[instruction()]
pub struct InitializeMargin<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
      init,
      payer = user,
      seeds = [b"margin", market.key().as_ref(), user.key().as_ref()],
      bump,
      space = 8 + std::mem::size_of::<MarginAccount>(),
    )]
    pub margin: Account<'info, MarginAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_margin(ctx: Context<InitializeMargin>) -> Result<()> {
    let m = &mut ctx.accounts.margin;
    m.owner = ctx.accounts.user.key();
    m.collateral = 0;
    m.margin_type = MarginType::Cross;
    m.positions = Vec::new();
    m.bump = ctx.bumps.margin;
    Ok(())
}
