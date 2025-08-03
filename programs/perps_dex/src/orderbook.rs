use crate::errors::ErrorCode;
use crate::slab::Slab;
use crate::state::{Market, OrderbookSide, Side};
use anchor_lang::prelude::*;
use anchor_lang::AnchorDeserialize;

pub const NULL_INDEX: u32 = u32::MAX;
pub const MAX_SLAB_CAPACITY: usize = 140;

#[derive(Accounts)]
#[instruction(side: u8)]
pub struct InitializeOrderbook<'info> {
    /// Metadata for this side (market, side, next_order_id, bump, etc)
    #[account(
        init,
        payer = authority,
        seeds = [b"orderbook", market.key().as_ref(), &[side]],
        bump,
        space = 8 + std::mem::size_of::<OrderbookSide>()
    )]
    pub orderbook_side: Account<'info, OrderbookSide>,

    /// The zero-copy slab buffer
    #[account(
        init,
        payer = authority,
        seeds = [b"slab", orderbook_side.key().as_ref()],
        bump,
        // 8 byte discriminator + full Slab size
        space = 8 + std::mem::size_of::<Slab>()
    )]
    pub slab: AccountLoader<'info, Slab>,

    pub market: Account<'info, Market>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_orderbook(
    ctx: Context<InitializeOrderbook>,
    side: u8,
    capacity: u32,
) -> Result<()> {
    msg!(
        "Starting initialize_orderbook: side={}, capacity={}",
        side,
        capacity
    );
    require!(
        (capacity as usize) > 0 && (capacity as usize) <= MAX_SLAB_CAPACITY,
        ErrorCode::InvalidOrderbookCapacity
    );

    // 1) Fill out your metadata account
    let ob = &mut ctx.accounts.orderbook_side;
    ob.market = ctx.accounts.market.key();
    ob.side = match side {
        0 => Side::Bid,
        1 => Side::Ask,
        _ => return Err(error!(ErrorCode::InvalidOrderbookSide)),
    };
    ob.next_order_id = 1u128;
    ob.bump = ctx.bumps.orderbook_side;

    // 2) Initialize the slab in-place
    let mut slab = ctx.accounts.slab.load_init()?;
    slab.init(capacity as usize, ob.side as u8)?;

    msg!("Initialized orderbook side: {:?}", ob.side);
    msg!(
        "Slab head={}, free_head={}, capacity={}",
        slab.head,
        slab.free_head,
        capacity
    );

    Ok(())
}
