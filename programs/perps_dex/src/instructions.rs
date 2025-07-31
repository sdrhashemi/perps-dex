use crate::state::{
    EventQueue, Governance, MarginAccount, Market, OrderbookSide, Proposal, Side, StakeAccount,
};
use anchor_lang::prelude::*;
use anchor_spl::token::Token;

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
#[derive(Accounts)]
#[instruction(side: Side)]
pub struct InitializeOrderbook<'info> {
    #[account(
        init,
        payer = authority,
        seeds = [b"orderbook", market.key().as_ref(), &[side as u8]],
        bump,
        space = 8 + std::mem::size_of::<OrderbookSide>(),
    )]
    pub orderbook_side: Account<'info, OrderbookSide>,
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction()]
pub struct InitializeEventQueue<'info> {
    #[account(
        init,
        payer = authority,
        seeds = [b"eventqueue", market.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<EventQueue>(),
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
#[instruction(side: Side)]
pub struct PlaceLimitOrder<'info> {
    #[account(
        mut,
        seeds = [b"orderbook", market.key().as_ref(), &[side as u8]],
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
    /// CHECK:
    #[account(mut)]
    pub oracle_pyth: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    pub oracle_switchboard: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,
    #[account(mut)]
    pub margin: Account<'info, MarginAccount>,
    #[account(mut)]
    pub orderbook_side: Account<'info, OrderbookSide>,
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

#[derive(Accounts)]
pub struct UpdateRiskParams<'info> {
    #[account(mut, has_one = authority)]
    pub market: Account<'info, Market>,
    pub authority: Signer<'info>,
}

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

// Governance Token Initialization
#[derive(Accounts)]
pub struct InitializeGovernance<'info> {
    #[account(init, payer = authority, space = 8 + std::mem::size_of::<Governance>())]
    pub governance: Account<'info, Governance>,
    #[account(init, payer = authority, mint::decimals = 6, mint::authority = governance)]
    pub governance_mint: Account<'info, anchor_spl::token::Mint>,
    #[account(init, payer = authority, token::mint = governance_mint, token::authority = governance)]
    pub governance_vault: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub rent: Sysvar<'info, Rent>,
}

// Stake Tokens
#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub governance: Account<'info, Governance>,
    #[account(mut)]
    pub governance_vault: Account<'info, anchor_spl::token::TokenAccount>,
    #[account(mut, seeds = [b"stake", user.key().as_ref()], bump)]
    pub stake_account: Account<'info, StakeAccount>,
    pub user: Signer<'info>,
    #[account(mut, constraint = user_vault.owner == user.key())]
    pub user_vault: Account<'info, anchor_spl::token::TokenAccount>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

// Propose Parameter Change
#[derive(Accounts)]
pub struct ProposeChange<'info> {
    #[account(mut)]
    pub governance: Account<'info, Governance>,
    #[account(init, payer = proposer, space = 8 + std::mem::size_of::<Proposal>())]
    pub proposal: Account<'info, Proposal>,
    #[account(mut)]
    pub proposer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Vote on Proposal
#[derive(Accounts)]
pub struct Vote<'info> {
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
    #[account(mut, seeds = [b"stake", voter.key().as_ref()], bump)]
    pub stake_account: Account<'info, StakeAccount>,
    pub voter: Signer<'info>,
}

// Execute Proposal
#[derive(Accounts)]
pub struct ExecuteProposal<'info> {
    #[account(mut)]
    pub governance: Account<'info, Governance>,
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
    #[account(mut, address = proposal.governance)]
    pub governance_account: Account<'info, Governance>,
    pub executor: Signer<'info>,
}
