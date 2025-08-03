use crate::errors::ErrorCode;
use crate::instructions::{InitializeEventQueue, UpdateRiskParams};

use crate::state::{EventQueue, MarketParams, OrderEvent};
use anchor_lang::prelude::*;
use anchor_lang::AnchorSerialize;
use pyth_sdk_solana::state::SolanaPriceAccount;
use switchboard_on_demand::PullFeedAccountData;

const MAX_DEVIATION_BPS: i128 = 50;

pub fn get_switchboard_price(
    feed_account: &AccountInfo,
    max_stale_slots: u64,
    min_samples: u32,
) -> Result<i128> {
    let account_data = feed_account.data.borrow();
    let feed = PullFeedAccountData::parse(account_data)
        .map_err(|_| error!(ErrorCode::InvalidPriceFeed))?;
    let price = feed
        .get_value(&Clock::get()?, max_stale_slots, min_samples, true)
        .map_err(|_| error!(ErrorCode::InvalidPriceFeed))?;
    let price = price
        .mantissa()
        .checked_mul(10i128.pow(price.scale() as u32))
        .ok_or_else(|| error!(ErrorCode::InvalidPriceFeed))?;
    Ok(price)
}

pub fn get_mark_price(
    pyth_account: &mut AccountInfo,
    switchboard_account: &mut AccountInfo,
    max_age: u64,
    max_stale_slots: u64,
    min_samples: u32,
) -> Result<i128> {
    let pyth_feed = SolanaPriceAccount::account_info_to_feed(pyth_account)
        .map_err(|_| error!(ErrorCode::InvalidPriceFeed))?;
    let clock = Clock::get()?;
    let pyth_data = pyth_feed
        .get_price_no_older_than(clock.unix_timestamp, max_age)
        .ok_or(error!(ErrorCode::StalePrice))?;
    let pyth_price = pyth_data.price as i128;

    let sb_price = get_switchboard_price(&switchboard_account, max_stale_slots, min_samples)?;

    let diff = (pyth_price - sb_price).abs();
    let deviation = if pyth_price != 0 {
        diff.saturating_mul(10_000.into()) / pyth_price
    } else {
        10_000.into()
    };

    if deviation <= MAX_DEVIATION_BPS {
        Ok((pyth_price + sb_price) / 2)
    } else {
        Ok(pyth_price)
    }
}

pub fn push_event(
    queue: &mut Account<EventQueue>,
    event_type: u8,
    key: u128,
    price: u64,
    qty: u64,
    owner: Pubkey,
) -> Result<()> {
    let event = OrderEvent {
        event_type,
        key,
        price,
        qty,
        owner,
    };
    let data = event
        .try_to_vec()
        .map_err(|_| error!(ErrorCode::EventSerializationFailure))?;
    queue.events.extend_from_slice(&data);
    queue.tail = queue.tail.wrapping_add(1);
    if queue.tail == queue.head {
        queue.head = queue.head.wrapping_add(1);
    }
    Ok(())
}

pub fn update_risk_params(ctx: Context<UpdateRiskParams>, new_params: MarketParams) -> Result<()> {
    let m = &mut ctx.accounts.market;
    m.params = new_params;
    Ok(())
}

pub fn initialize_event_queue(ctx: Context<InitializeEventQueue>) -> Result<()> {
    let eq = &mut ctx.accounts.event_queue;
    eq.market = ctx.accounts.market.key();
    eq.head = 0;
    eq.tail = 0;
    eq.events = Vec::new();
    eq.bump = ctx.bumps.event_queue;
    Ok(())
}
