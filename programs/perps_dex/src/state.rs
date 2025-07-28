use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MarketParams {
    pub tick_size: u64,
    pub lot_size: u64,
    pub leverage_limit: u8,
    pub funding_interval: i64,
}

#[account]
pub struct Market {
    pub authority: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    
    pub oracle_pyth: Pubkey,
    pub oracle_switchboard: Pubkey,
    pub params: MarketParams,
    pub nonce: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

#[account]
pub struct OrderbookSide {
    pub market: Pubkey,
    pub side: Side,
    pub head: u32,
    pub free_head: u32,
    pub slab: Vec<u8>,
    pub next_order_id: u128,
    pub bump: u8,
}

#[account]
pub struct EventQueue {
    pub market: Pubkey,
    pub head: u32,
    pub tail: u32,
    pub events: Vec<u8>,
    pub bump: u8,
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone)]
#[repr(C)]
pub struct OrderEvent {
    pub event_type: u8,
    pub key: u128,
    pub price: u64,
    pub qty: u64,
    pub owner: Pubkey,
}

#[account]
pub struct MarginAccount {
    pub owner: Pubkey,
    pub collateral: u64,
    pub positions: Vec<Position>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Position {
    pub key: u128,
    pub qty: u64,
    pub entry_price: u64,
    pub side: Side,
}
