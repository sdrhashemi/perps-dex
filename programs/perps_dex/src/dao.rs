use anchor_lang::prelude::*;
use anchor_spl::token::{self, MintTo, Transfer};

use crate::state::{Governance, MarketParams, Proposal, StakeAccount};
use crate::errors::ErrorCode;

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

pub fn initialize_governance(ctx: Context<InitializeGovernance>, total_supply: u64) -> Result<()> {
    let governance_ai = ctx.accounts.governance.to_account_info();

    let gov = &mut ctx.accounts.governance;
    gov.authority = ctx.accounts.authority.key();
    gov.mint = ctx.accounts.governance_mint.key();
    gov.vault = ctx.accounts.governance_vault.key();

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        MintTo {
            mint: ctx.accounts.governance_mint.to_account_info(),
            to: ctx.accounts.governance_vault.to_account_info(),
            authority: governance_ai,
        },
    );
    token::mint_to(
        cpi_ctx.with_signer(&[&[b"governance", &[gov.bump]]]),
        total_supply,
    )?;
    Ok(())
}

pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);
    let cpi_accounts = Transfer {
        from: ctx.accounts.user_vault.to_account_info(),
        to: ctx.accounts.governance_vault.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, amount)?;

    let st = &mut ctx.accounts.stake_account;
    st.user = ctx.accounts.user.key();
    st.amount = st.amount.saturating_add(amount);
    Ok(())
}

pub fn propose_change(
    ctx: Context<ProposeChange>,
    new_params: MarketParams,
    description: String,
) -> Result<()> {
    let p = &mut ctx.accounts.proposal;
    p.governance = ctx.accounts.governance.key();
    p.proposer = ctx.accounts.proposer.key();
    p.new_params = new_params;
    p.description = description;
    p.votes_for = 0;
    p.votes_against = 0;
    p.executed = false;
    Ok(())
}

pub fn vote(ctx: Context<Vote>, approve: bool) -> Result<()> {
    let st = &ctx.accounts.stake_account;
    require!(
        !ctx.accounts.proposal.executed,
        ErrorCode::ProposalAlreadyExecuted
    );
    if approve {
        ctx.accounts.proposal.votes_for = ctx
            .accounts
            .proposal
            .votes_for
            .saturating_add(st.amount as u64);
    } else {
        ctx.accounts.proposal.votes_against = ctx
            .accounts
            .proposal
            .votes_against
            .saturating_add(st.amount as u64);
    }
    Ok(())
}

pub fn execute_proposal(ctx: Context<ExecuteProposal>) -> Result<()> {
    let p = &mut ctx.accounts.proposal;
    require!(!p.executed, ErrorCode::ProposalAlreadyExecuted);
    require!(p.votes_for > p.votes_against, ErrorCode::ProposalNotPassed);
    let gov = &mut ctx.accounts.governance;
    gov.params = p.new_params.clone();
    p.executed = true;
    Ok(())
}

