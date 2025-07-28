use crate::errors::ErrorCode;
use crate::instructions::{
    InitializeMarket, Liquidate, PlaceLimitOrder, PlaceMarketOrder, SettleFunding, UpdateRiskParams,
};
use crate::orderbook::Slab;
use crate::state::{EventQueue, MarginType, MarketParams, OrderEvent, Side};
use anchor_lang::prelude::*;
use anchor_lang::AnchorDeserialize;
use anchor_lang::AnchorSerialize;
use anchor_spl::token::{self, Transfer};
use pyth_sdk_solana::state::SolanaPriceAccount;
use rust_decimal::Decimal;
use switchboard_on_demand::PullFeedAccountData;

fn decode_slab(data: &[u8], head: u32, free_head: u32) -> Slab {
    let node_size = std::mem::size_of::<crate::orderbook::SlabNode>();
    let capacity = data.len() / node_size;
    let mut slab = Slab::new(capacity);
    let tmp: Vec<crate::orderbook::SlabNode> =
        <Vec<crate::orderbook::SlabNode> as AnchorDeserialize>::try_from_slice(data)
            .unwrap_or_else(|_| vec![crate::orderbook::SlabNode::default(); capacity]);
    slab.nodes.copy_from_slice(&tmp);
    slab.head = head;
    slab.free_head = free_head;
    slab
}

fn encode_slab(slab: &Slab) -> (Vec<u8>, u32, u32) {
    let bytes = slab.nodes.try_to_vec().unwrap();
    (bytes, slab.head, slab.free_head)
}
fn get_switchboard_price(
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

fn get_mark_price(
    pyth_account: &mut AccountInfo,
    switchboard_account: &mut AccountInfo,
    max_age: u64,
    max_stale_slots: u64,
    min_samples: u32,
) -> Result<i128> {
    if let Ok(pyth_feed) = SolanaPriceAccount::account_info_to_feed(pyth_account) {
        let clock = Clock::get()?;
        if let Some(price_data) = pyth_feed.get_price_no_older_than(clock.unix_timestamp, max_age) {
            return Ok(price_data.price as i128);
        }
    }
    let price = get_switchboard_price(&switchboard_account, max_stale_slots, min_samples)?;
    Ok(price)
}

fn push_event(
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
pub fn initialize_market(
    ctx: Context<InitializeMarket>,
    market_nonce: u8,
    params: MarketParams,
) -> Result<()> {
    let m = &mut ctx.accounts.market;
    m.authority = *ctx.accounts.authority.key;
    m.base_mint = ctx.accounts.base_mint.key();
    m.quote_mint = ctx.accounts.quote_mint.key();
    m.oracle_pyth = Pubkey::default();
    m.oracle_switchboard = Pubkey::default();
    m.params = params;
    m.nonce = market_nonce;
    Ok(())
}

pub fn place_limit_order(
    ctx: Context<PlaceLimitOrder>,
    price: u64,
    qty: u64,
    side: Side,
    _reduce_only: bool,
) -> Result<()> {
    let ob = &mut ctx.accounts.orderbook_side;
    let mut slab = decode_slab(&ob.slab, ob.head, ob.free_head);
    let key = ob.next_order_id as u128;
    slab.insert(key, price, qty, ctx.accounts.user.key(), side)?;
    ob.next_order_id = ob
        .next_order_id
        .checked_add(1)
        .ok_or(ErrorCode::OrderbookOverflow)?;
    let (bytes, head, free_head) = encode_slab(&slab);
    ob.slab = bytes;
    ob.head = head;
    ob.free_head = free_head;
    push_event(
        &mut ctx.accounts.event_queue,
        0,
        key,
        price,
        qty,
        ctx.accounts.user.key(),
    )?;
    Ok(())
}

pub fn place_market_order(ctx: Context<PlaceMarketOrder>, qty: u64, _side: Side) -> Result<()> {
    let ob = &mut ctx.accounts.orderbook_side;
    let mut slab = decode_slab(&ob.slab, ob.head, ob.free_head);
    let mut remaining = qty;
    while remaining > 0 {
        if let Some(idx) = slab.find_best() {
            let (order_key, price, avail, owner) = {
                let node = &slab.nodes[idx as usize];
                (node.key, node.price, node.qty, node.owner)
            };
            let trade_qty = remaining.min(avail);
            slab.reduce_order(idx, trade_qty)?;
            push_event(
                &mut ctx.accounts.event_queue,
                1,
                order_key,
                price,
                trade_qty,
                owner,
            )?;
            remaining = remaining.saturating_sub(trade_qty);
        } else {
            break;
        }
    }
    let (bytes, head, free_head) = encode_slab(&slab);
    ob.slab = bytes;
    ob.head = head;
    ob.free_head = free_head;
    Ok(())
}

pub fn settle_funding(ctx: Context<SettleFunding>) -> Result<()> {
    let mut pyth_ai = ctx.accounts.oracle_pyth.to_account_info();
    let mut sb_ai = ctx.accounts.oracle_switchboard.to_account_info();
    let max_age = ctx.accounts.market.params.funding_interval;
    let mark_price = get_mark_price(&mut pyth_ai, &mut sb_ai, max_age, 5, 3)?;

    // branch by margin mode
    let margin = &mut ctx.accounts.margin;
    match margin.margin_type {
        // cross margin
        MarginType::Cross => {
            let mut net_funding: i128 = 0;
            for pos in margin.positions.iter() {
                let entry = pos.entry_price as i128;
                let diff = mark_price.saturating_sub(entry);
                let funding = diff
                    .saturating_mul(pos.qty as i128)
                    .checked_div(entry)
                    .unwrap_or(0);
                net_funding = match pos.side {
                    Side::Bid => net_funding.saturating_sub(funding),
                    Side::Ask => net_funding.saturating_add(funding),
                };
            }
            if net_funding < 0 {
                let deduct = (-net_funding) as u64;
                require!(
                    margin.collateral >= deduct,
                    ErrorCode::InsufficientCollateral
                );
                margin.collateral = margin.collateral.saturating_sub(deduct);
            } else {
                margin.collateral = margin.collateral.saturating_add(net_funding as u64);
            }
        }
        // isolated margin
        MarginType::Isolated => {
            for pos in margin.positions.iter_mut() {
                let entry = pos.entry_price as i128;
                let diff = mark_price.saturating_sub(entry);
                let funding_i = diff
                    .saturating_mul(pos.qty as i128)
                    .checked_div(entry)
                    .unwrap_or(0);
                let delta = if pos.side == Side::Bid {
                    (funding_i as i128).saturating_neg() as i64 as u64
                } else {
                    funding_i as u64
                };

                require!(pos.collateral >= delta, ErrorCode::InsufficientCollateral);
                pos.collateral = pos.collateral.saturating_add(if pos.side == Side::Ask {
                    delta
                } else {
                    delta.wrapping_neg()
                });
            }
        }
    }
    Ok(())
}

pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
    let mut price_account = ctx.accounts.oracle_pyth.to_account_info();
    let price_feed = SolanaPriceAccount::account_info_to_feed(&mut price_account)
        .map_err(|_| error!(ErrorCode::InvalidPriceFeed))?;
    let clock = Clock::get()?;
    let price_data = price_feed
        .get_price_no_older_than(
            clock.unix_timestamp,
            ctx.accounts.market.params.funding_interval,
        )
        .ok_or(error!(ErrorCode::StalePrice))?;
    let mark_price = price_data.price as i128;

    // compute equity & notional
    let margin = &mut ctx.accounts.margin;
    let mut equity: i128 = margin.collateral as i128;
    let mut notional: i128 = 0;
    for pos in margin.positions.iter() {
        let entry = pos.entry_price as i128;
        let side_sign = match pos.side {
            Side::Bid => 1,
            Side::Ask => -1,
        };
        let pnl = (mark_price - entry).saturating_mul(pos.qty as i128) * side_sign;
        equity = equity.saturating_add(pnl);
        notional = notional.saturating_add(entry.saturating_mul(pos.qty as i128));
    }

    // health check
    let health_ratio = if notional > 0 {
        equity.saturating_mul(10_000) / notional
    } else {
        0
    };
    let threshold = ctx.accounts.market.params.maintenance_margin_ratio as i128;
    require!(health_ratio < threshold, ErrorCode::HealthyAccount);

    // unwind positions
    let mut slab = decode_slab(
        &ctx.accounts.orderbook_side.slab,
        ctx.accounts.orderbook_side.head,
        ctx.accounts.orderbook_side.free_head,
    );
    let mut total_proceeds: u64 = 0;
    for pos in margin.positions.iter() {
        let mut remaining = pos.qty;
        while remaining > 0 {
            if let Some(idx) = slab.find_best() {
                let (price, available_qty) = {
                    let node_ref = &slab.nodes[idx as usize];
                    (node_ref.price, node_ref.qty)
                };
                let trade_qty = remaining.min(available_qty);
                slab.reduce_order(idx, trade_qty)?;
                total_proceeds = total_proceeds.saturating_add(
                    (trade_qty as u128)
                        .saturating_mul(price as u128)
                        .try_into()
                        .unwrap(),
                );
                remaining = remaining.saturating_sub(trade_qty);
            } else {
                break;
            }
        }
    }

    // re‐persist slab
    let (bytes, head, free_head) = encode_slab(&slab);
    let ob = &mut ctx.accounts.orderbook_side;
    ob.slab = bytes;
    ob.head = head;
    ob.free_head = free_head;

    // collect and distribute fees
    let fee = total_proceeds / 200; // 0.5%
    let liquidator_share = fee;
    let vault_share = total_proceeds.saturating_sub(fee);

    // transfer the penalty to the liquidator
    let cpi_accounts = Transfer {
        from: ctx.accounts.collateral_vault.to_account_info(),
        to: ctx.accounts.liquidator_collateral_account.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, liquidator_share)?;

    // clear positions
    margin.positions.clear();
    margin.collateral = vault_share;

    Ok(())
}

pub fn update_risk_params(ctx: Context<UpdateRiskParams>, new_params: MarketParams) -> Result<()> {
    let m = &mut ctx.accounts.market;
    m.params = new_params;
    Ok(())
}
