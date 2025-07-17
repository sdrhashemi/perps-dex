use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use crate::state::{NonZeroPubkeyOption, OracleConfig};

pub struct Market {
    pub bump: u8,

    pub base_decimals: u8,
    pub quote_decimals: u8,

    pub padding: [u8; 5],

    /// PDA for signing vault txs
    pub market_authority: Pubkey,

    /// No expiry = 0. Market will expire and no trading allowed after time_expiry
    pub time_expiry: i64,

    pub collect_fee_admin: Pubkey,
    pub open_orders_admin: NonZeroPubkeyOption,
    pub consume_events_admin: NonZeroPubkeyOption,
    pub close_market_admin: NonZeroPubkeyOption,

    pub name: [u8; 16],

    /// Address of the BookSide account for bids
    pub bids: Pubkey,
    /// Address of the BookSide account for asks
    pub asks: Pubkey,
    /// Address of the EventHeap account
    pub event_heap: Pubkey

    pub oracle_a: NonZeroPubkeyOption,
    pub oracle_b: NonZeroPubkeyOption,
    /// Oracle Configurations
    pub oracle_config: OracleConfig,


    pub quote_lot_size: i64,

    pub base_lot_size: i64,

    pub seq_num: u64,

    pub registration_time: i64,

    /// Fees
    pub maker_fee: i64,
    pub taker_fee: i64,

    pub fees_accrued: u128,
    pub fees_to_referrers: u128,

    pub referrer_rebates_accrued: u64,

    pub fees_available: u64,

    /// Cumulative maker volume (same as taker volume) in quote native units
    pub maker_volume: u128,

    /// Cumulative taker volume in quote native units due to place take orders
    pub taker_volume_wo_oo: u128,

    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,

    pub market_base_vault: Pubkey,
    pub base_deposit_total: u64,

    pub market_quote_vault: Pubkey,
    pub quote_deposit_total: u64,

    pub reserved: [u8; 128],
}

impl Market {
    pub const LEN: usize = 1  // bump
        + 1  // base_decimals
        + 1  // quote_decimals
        + 5  // padding1
        + 32 // market_authority
        + 8  // time_expiry
        + 32 // collect_fee_admin
        + (32 * 3) // three NonZeroPubkeyOption fields
        + 16 // name
        + (32 * 3) // bids, asks, event_heap
        + (32 * 2) // oracles
        + OracleConfig::LEN // oracle_config
        + 8  // quote_lot_size
        + 8  // base_lot_size
        + 8  // seq_num
        + 8  // registration_time
        + 8 * 2 // maker_fee, taker_fee
        + 16 * 2 // fees_accrued, fees_to_referrers (u128)
        + 8  // referrer_rebates_accrued
        + 8  // fees_available
        + 16 * 2 // maker_volume, taker_volume_wo_oo
        + (32 * 2) // base_mint, quote_mint
        + (32 * 2) // vaults
        + 8  // base_deposit_total
        + 8  // quote_deposit_total
        + 128; // reserved
}
