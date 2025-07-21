use anchor_lang::prelude::*;
use crate::state::*;

#[derive(Accounts)]
#[instruction(market_nonce: u8, params: MarketParams)]
pub struct InitializeMarket<'info> {
    #[account(
        init,
        seeds = [b"market", base_mint.key().as_ref(), quote_mint.key().as_ref(), &[market_nonce]],
        bump,
        payer = authority,
        space = 8 + std::mem::size_of::<Market>(),
    )]
    pub market: Account<'info, Market>,
    pub base_mint: AccountInfo<'info>,
    pub quote_mint: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlaceLimitOrder<'info> {
    #[account(
        mut,
        seeds = [b"orderbook", market.key().as_ref(), &[orderbook_side.bump]],
        bump = orderbook_side.bump
    )]
    pub orderbook_side: Account<'info, OrderbookSide>,

    #[account(
        mut,
        seeds = [b"eventqueue", market.key().as_ref()],
        bump = event_queue.bump
    )]
    pub event_queue: Account<'info, EventQueue>,

    #[account(mut)] pub user: Signer<'info>,
    #[account(mut)] pub margin: Account<'info, MarginAccount>,
    pub system_program: Program<'info, System>,
    pub market: Account<'info, Market>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

#[derive(Accounts)]
pub struct PlaceMarketOrder<'info> {
    #[account(
        mut,
        seeds = [b"orderbook", market.key().as_ref(), &[orderbook_side.bump]],
        bump = orderbook_side.bump,
    )]
    pub ordedrbook_side: Account<'info, OrderbookSide>,

    #[account(
        mut,
        seeds = [b"eventqueue", market.key().as_ref(), &[orderbook_side.bump]],
        bump = orderbook_side.bump
    )]
    pub event_queue: Account<'info, EventQueue>,

    #[account(mut)] pub user: Signer<'info>,
    #[account(mut)] pub margin: Account<'info, MarginAccount>,
    pub market: Account<'info, Market>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}
