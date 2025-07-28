use crate::errors::ErrorCode;
use crate::instructions::{
    InitializeMarket, Liquidate, PlaceLimitOrder, PlaceMarketOrder, SettleFunding, UpdateRiskParams,
};
use crate::orderbook::Slab;
use crate::state::{EventQueue, MarginAccount, Market, MarketParams, OrderEvent, OrderbookSide, Side};
use anchor_lang::prelude::*;
use anchor_lang::AnchorDeserialize;
use anchor_lang::AnchorSerialize;

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

fn push_event(queue: &mut Account<EventQueue>, event_type: u8, key: u128, price: u64, qty: u64, owner: Pubkey) {
    let event = OrderEvent {
        event_type,
        key,
        price,
        qty,
        owner,
    };
    let data = event.try_to_vec().expect("Event serialization failed");
    queue.events.extend_from_slice(&data);
    queue.tail = queue.tail.wrapping_add(1);
    if queue.tail == queue.head {
        queue.head = queue.head.wrapping_add(1);
    }
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
    push_event(&mut ctx.accounts.event_queue, 0, key, price, qty, ctx.accounts.user.key());
    Ok(())
}

pub fn place_market_order(ctx: Context<PlaceMarketOrder>, qty: u64, _side: Side) -> Result<()> {
    let ob = &mut ctx.accounts.orderbook_side;
    let mut slab = decode_slab(&ob.slab, ob.head, ob.free_head);
    let mut remaining = qty;
    while remaining > 0 {
        if let Some(best_idx) = slab.find_best() {
            let node = &slab.nodes[best_idx as usize];
            let trade_qty = remaining.min(node.qty);
            slab.reduce_order(best_idx, trade_qty)?;
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
    let margin = &mut ctx.accounts.margin;
    let fee: u64 = 1;
    require!(margin.collateral >= fee, ErrorCode::InsufficientCollateral);
    margin.collateral = margin.collateral.saturating_sub(fee);
    Ok(())
}

pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
    let margin = &mut ctx.accounts.margin;
    let penalty = margin.collateral / 10;
    margin.collateral = margin.collateral.saturating_sub(penalty);
    margin.positions.clear();
    Ok(())
}

pub fn update_risk_params(ctx: Context<UpdateRiskParams>, new_params: MarketParams) -> Result<()> {
    let m = &mut ctx.accounts.market;
    m.params = new_params;
    Ok(())
}
