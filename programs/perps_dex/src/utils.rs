use crate::errors::ErrorCode;
use crate::instructions::{
    DepositCollateral, ExecuteProposal, InitializeGovernance, InitializeMarket, Liquidate,
    PlaceLimitOrder, PlaceMarketOrder, ProposeChange, SettleFills, SettleFunding, Stake,
    UpdateRiskParams, Vote, WithdrawCollateral,
};
use crate::orderbook::Slab;
use crate::state::{EventQueue, MarginType, MarketParams, OrderEvent, Position, Side};
use anchor_lang::prelude::*;
use anchor_lang::AnchorDeserialize;
use anchor_lang::AnchorSerialize;
use anchor_spl::associated_token::spl_associated_token_account::tools::account;
use anchor_spl::token::{self, MintTo, Transfer};
use pyth_sdk_solana::state::SolanaPriceAccount;
use switchboard_on_demand::PullFeedAccountData;

const MAX_DEVIATION_BPS: i128 = 50;

pub fn decode_slab(data: &[u8], head: Option<u32>, free_head: Option<u32>, side: Side) -> Slab {
    let node_size = std::mem::size_of::<crate::orderbook::SlabNode>();
    let capacity = data.len() / node_size;
    let mut slab = Slab::new(capacity, side);
    let tmp: Vec<crate::orderbook::SlabNode> =
        Vec::<crate::orderbook::SlabNode>::try_from_slice(data)
            .unwrap_or_else(|_| vec![crate::orderbook::SlabNode::default(); capacity]);
    slab.nodes.clone_from_slice(&tmp);
    slab.head = head;
    slab.free_head = free_head;
    slab
}

pub fn encode_slab(slab: &Slab) -> (Vec<u8>, u32, u32) {
    let bytes = slab.nodes.try_to_vec().unwrap();
    (
        bytes,
        slab.head.unwrap_or_default(),
        slab.free_head.unwrap_or_default(),
    )
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
    _side: Side,
    _reduce_only: bool,
) -> Result<()> {
    let market = &ctx.accounts.market;
    let margin = &ctx.accounts.margin;

    let collateral: u128 = margin.collateral as u128;
    let lev_limit: u128 = market.params.leverage_limit as u128;

    let existing_notional: u128 = margin
        .positions
        .iter()
        .map(|p| (p.entry_price as u128) * (p.qty as u128))
        .sum();

    let order_notional = (price as u128) * (qty as u128);
    let total_notional = existing_notional
        .checked_add(order_notional)
        .ok_or(error!(ErrorCode::Overflow))?;

    require!(
        total_notional <= collateral.saturating_mul(lev_limit),
        ErrorCode::LeverageExceeded
    );

    let ob = &mut ctx.accounts.orderbook_side;
    let mut slab = decode_slab(&ob.slab, Some(ob.head), Some(ob.free_head), ob.side.clone());
    let key = ob.next_order_id as u128;
    slab.insert(key, price, qty, ctx.accounts.user.key())?;
    ob.next_order_id = ob
        .next_order_id
        .checked_add(1)
        .ok_or(error!(ErrorCode::OrderbookOverflow))?;
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
    let mut slab = decode_slab(&ob.slab, Some(ob.head), Some(ob.free_head), ob.side.clone());
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
    let now = ctx.accounts.clock.unix_timestamp;
    let mut pyth_ai = ctx.accounts.oracle_pyth.to_account_info();
    let mut sb_ai = ctx.accounts.oracle_switchboard.to_account_info();
    let max_age = ctx.accounts.market.params.funding_interval;
    let mark_price = get_mark_price(&mut pyth_ai, &mut sb_ai, max_age, 5, 3)?;

    // branch by margin mode
    let m = &mut ctx.accounts.margin;
    match m.margin_type {
        MarginType::Cross => {
            let mut net: i128 = 0;
            for pos in &m.positions {
                let e = pos.entry_price as i128;
                let diff = mark_price.saturating_sub(e);
                let fund = diff
                    .saturating_mul(pos.qty as i128)
                    .checked_div(e)
                    .unwrap_or(0);
                net = match pos.side {
                    Side::Bid => net.saturating_sub(fund),
                    Side::Ask => net.saturating_add(fund),
                };
            }
            if net < 0 {
                let d = (-net) as u64;
                require!(m.collateral >= d, ErrorCode::InsufficientCollateral);
                m.collateral = m.collateral.saturating_sub(d);
            } else {
                m.collateral = m.collateral.saturating_add(net as u64);
            }
        }
        MarginType::Isolated => {
            for pos in &mut m.positions {
                let e = pos.entry_price as i128;
                let diff = mark_price.saturating_sub(e);
                let fund = diff
                    .saturating_mul(pos.qty as i128)
                    .checked_div(e)
                    .unwrap_or(0);
                let delta = if pos.side == Side::Bid {
                    (fund as i128).saturating_neg() as i64 as u64
                } else {
                    fund as u64
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

    // 4) update timestamp
    ctx.accounts.market.last_funding_timestamp = now;
    Ok(())
}

pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
    let mut price_account = ctx.accounts.oracle_pyth.clone();
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
        let sign = if pos.side == Side::Bid { 1 } else { -1 };
        let pnl = (mark_price - entry).saturating_mul(pos.qty as i128) * sign;
        equity = equity.saturating_add(pnl);
        notional = notional.saturating_add((entry.saturating_mul(pos.qty as i128)).abs());
    }

    // health check against maintenance margin ratio
    let health = if notional > 0 {
        equity.saturating_mul(10_000) / notional
    } else {
        0
    };
    let threshold = ctx.accounts.market.params.maintenance_margin_ratio as i128;
    require!(health < threshold, ErrorCode::HealthyAccount);

    // unwind positions via orderbook
    let mut slab = decode_slab(
        &ctx.accounts.orderbook_side.slab,
        Some(ctx.accounts.orderbook_side.head),
        Some(ctx.accounts.orderbook_side.free_head),
        ctx.accounts.orderbook_side.side.clone(),
    );
    let mut total_proceeds: u128 = 0;
    for pos in margin.positions.iter() {
        let mut rem = pos.qty;
        while rem > 0 {
            let idx = match slab.find_best() {
                Some(i) => i,
                None => break,
            };
            let (price, available_qty) = {
                let node_ref = &slab.nodes[idx as usize];
                (node_ref.price, node_ref.qty)
            };
            let trade_qty = rem.min(available_qty);
            slab.reduce_order(idx, trade_qty)?;
            total_proceeds =
                total_proceeds.saturating_add((trade_qty as u128).saturating_mul(price as u128));
            rem = rem.saturating_sub(trade_qty);
        }
    }

    // persist updated slab
    let (bytes, head, free_head) = encode_slab(&slab);
    let ob = &mut ctx.accounts.orderbook_side;
    ob.slab = bytes;
    ob.head = head;
    ob.free_head = free_head;

    // apply 0.5% liquidation penalty
    let proceeds_u64: u64 = total_proceeds
        .try_into()
        .map_err(|_| error!(ErrorCode::Overflow))?;
    let fee = proceeds_u64 / 200;
    let liquidator_cut = fee;
    let vault_cut = proceeds_u64.saturating_sub(fee);

    // transfer liquidator's share
    let cpi_accounts = Transfer {
        from: ctx.accounts.collateral_vault.to_account_info(),
        to: ctx.accounts.liquidator_collateral_account.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, liquidator_cut)?;

    // clear positions and remaining collateral
    margin.positions.clear();
    margin.collateral = vault_cut;

    Ok(())
}

pub fn update_risk_params(ctx: Context<UpdateRiskParams>, new_params: MarketParams) -> Result<()> {
    let m = &mut ctx.accounts.market;
    m.params = new_params;
    Ok(())
}

pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let cpi_accounts = Transfer {
        from: ctx.accounts.user_collateral.to_account_info(),
        to: ctx.accounts.market_vault.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, amount)?;

    let margin = &mut ctx.accounts.margin;
    margin.collateral = margin.collateral.saturating_add(amount);

    Ok(())
}

pub fn withdraw_collateral(ctx: Context<WithdrawCollateral>, amount: u64) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let margin = &mut ctx.accounts.margin;
    let after = margin
        .collateral
        .checked_sub(amount)
        .ok_or(error!(ErrorCode::InsufficientCollateral))?;

    margin.collateral = after;

    let cpi_accounts = Transfer {
        from: ctx.accounts.market_vault.to_account_info(),
        to: ctx.accounts.user_collateral.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    token::transfer(cpi_ctx, amount)?;

    Ok(())
}

pub fn settle_fills(ctx: Context<SettleFills>) -> Result<()> {
    const ORDER_EVENT_SIZE: usize = 65;
    let queue = &mut ctx.accounts.event_queue;
    while queue.head != queue.tail {
        let idx = queue.head as usize;
        let start = idx
            .checked_mul(ORDER_EVENT_SIZE)
            .ok_or(error!(ErrorCode::Overflow))?;
        let end = start
            .checked_add(ORDER_EVENT_SIZE)
            .ok_or(error!(ErrorCode::Overflow))?;
        require!(
            end <= queue.events.len(),
            ErrorCode::EventDeserializationFailure
        );
        let data = &queue.events[start..end];
        let ev = OrderEvent::try_from_slice(data)
            .map_err(|_| error!(ErrorCode::EventDeserializationFailure))?;
        if ev.event_type == 1 {
            let amt_u128 = (ev.price as u128)
                .checked_mul(ev.qty as u128)
                .ok_or(error!(ErrorCode::Overflow))?;
            let amt: u64 = amt_u128
                .try_into()
                .map_err(|_| error!(ErrorCode::Overflow))?;
            let cpi_accounts = Transfer {
                from: ctx.accounts.taker_collateral.to_account_info(),
                to: ctx.accounts.maker_collateral.to_account_info(),
                authority: ctx.accounts.taker.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
            token::transfer(cpi_ctx, amt)?;
            let maker_margin = &mut ctx.accounts.maker_margin;
            if let Some(pos) = maker_margin.positions.iter_mut().find(|p| p.key == ev.key) {
                pos.qty = pos.qty.saturating_sub(ev.qty);
            }
            let taker_margin = &mut ctx.accounts.taker_margin;
            let taker_side = if ctx.accounts.orderbook_side.side == Side::Bid {
                Side::Ask
            } else {
                Side::Bid
            };
            if let Some(pos) = taker_margin.positions.iter_mut().find(|p| p.key == ev.key) {
                pos.qty = pos.qty.saturating_add(ev.qty);
            } else {
                taker_margin.positions.push(Position {
                    key: ev.key,
                    qty: ev.qty,
                    entry_price: ev.price,
                    side: taker_side,
                    collateral: 0,
                });
            }
        }
        queue.head = queue.head.wrapping_add(1);
    }
    // clean up the zero-qtys
    ctx.accounts.maker_margin.positions.retain(|p| p.qty > 0);
    ctx.accounts.taker_margin.positions.retain(|p| p.qty > 0);
    Ok(())
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
