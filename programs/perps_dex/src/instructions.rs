use crate::state::{EventQueue, MarginAccount, Market, OrderbookSide};
use anchor_lang::prelude::*;


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
        seeds = [b"orderbook", market.key().as_ref(), b"limit"],
        bump = orderbook_side.bump
    )]
    pub orderbook_side: Account<'info, OrderbookSide>,

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
pub struct PlaceMarketOrder<'info> {
    #[account(
        mut,
        seeds = [b"orderbook", market.key().as_ref(), b"limit"],
        bump = orderbook_side.bump
    )]
    pub orderbook_side: Account<'info, OrderbookSide>,

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

#[derive(Accounts)]
pub struct SettleFunding<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(mut)]
    pub margin: Account<'info, MarginAccount>,

    pub oracle_pyth: AccountInfo<'info>,
    pub oracle_switchboard: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(mut)]
    pub margin: Account<'info, MarginAccount>,
    
    #[account(mut)]
    pub liquidator: Signer<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

#[derive(Accounts)]
pub struct UpdateRiskParams<'info> {
    #[account(mut, has_one = authority)]
    pub market: Account<'info, Market>,

    pub authority: Signer<'info>,
}
