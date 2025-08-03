use crate::state::{EventQueue, MarginAccount, Market};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction()]
pub struct InitializeEventQueue<'info> {
    #[account(
        init,
        payer = authority,
        seeds = [b"eventqueue", market.key().as_ref()],
        bump,
        space = 8 + 5000,
    )]
    pub event_queue: Account<'info, EventQueue>,
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

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

#[derive(Accounts)]
pub struct UpdateRiskParams<'info> {
    #[account(mut, has_one = authority)]
    pub market: Account<'info, Market>,
    pub authority: Signer<'info>,
}

// Governance Token Initialization
