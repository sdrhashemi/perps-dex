use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::state::Side;

/// A node in the slab (orderbook), representing a single order
#[derive(Clone, AnchorSerialize, AnchorDeserialize, Default)]
pub struct SlabNode {
    pub key: u128,
    pub price: u64,
    pub qty: u64,
    pub owner: Pubkey,
    /// Slot when the order was inserted, for deterministic tiebreaking
    pub inserted_slot: u64,
    /// Index of the next node in this price level
    pub next: Option<u32>,
    /// Index of the previous node in this price level
    pub prev: Option<u32>,
}

/// On-chain slab managing orders for one side (bid or ask)
pub struct Slab {
    pub nodes: Vec<SlabNode>,
    /// Head points to the best order (None if empty)
    pub head: Option<u32>,
    /// Free list head (None if full)
    pub free_head: Option<u32>,
    /// Which side this slab represents
    pub side: Side,
}

impl Slab {
    /// Create a new empty slab with given capacity and side
    pub fn new(capacity: usize, side: Side) -> Self {
        let mut nodes = Vec::with_capacity(capacity);
        nodes.resize_with(capacity, || SlabNode::default());
        // initialize free list: 0 -> 1 -> ... -> capacity-1 -> None
        for i in 0..capacity - 1 {
            nodes[i].next = Some((i + 1) as u32);
        }
        nodes[capacity - 1].next = None;
        Self { nodes, head: None, free_head: Some(0), side }
    }

    /// Allocate a free node index
    fn allocate(&mut self) -> Result<u32> {
        let idx = self.free_head.ok_or(error!(ErrorCode::OrderbookOverflow))?;
        self.free_head = self.nodes[idx as usize].next;
        Ok(idx)
    }

    /// Insert a new order node, stamping with the current slot
    pub fn insert(&mut self, key: u128, price: u64, qty: u64, owner: Pubkey) -> Result<()> {
        let slot = Clock::get()?.slot;
        let idx = self.allocate()?;
        let node = &mut self.nodes[idx as usize];
        node.key = key;
        node.price = price;
        node.qty = qty;
        node.owner = owner;
        node.inserted_slot = slot;
        node.prev = None;
        node.next = self.head;
        if let Some(old_head) = self.head {
            self.nodes[old_head as usize].prev = Some(idx);
        }
        self.head = Some(idx);
        Ok(())
    }

    /// Reduce an order's quantity; free node if fully filled
    pub fn reduce_order(&mut self, idx: u32, qty: u64) -> Result<()> {
        let (prev, next, remaining) = {
            let n = &self.nodes[idx as usize];
            (n.prev, n.next, n.qty.saturating_sub(qty))
        };
        if remaining == 0 {
            if let Some(p) = prev {
                self.nodes[p as usize].next = next;
            } else {
                self.head = next;
            }
            if let Some(nxt) = next {
                self.nodes[nxt as usize].prev = prev;
            }
            self.nodes[idx as usize].next = self.free_head;
            self.free_head = Some(idx);
        } else {
            self.nodes[idx as usize].qty = remaining;
        }
        Ok(())
    }

    /// Whether this slab represents the bid side
    pub fn is_bid_side(&self) -> bool {
        self.side == Side::Bid
    }

    /// Find the best order index, breaking ties by earliest inserted_slot
    pub fn find_best(&self) -> Option<u32> {
        // determine best price
        let mut best_price: Option<u64> = None;
        for n in &self.nodes {
            if n.qty == 0 { continue; }
            best_price = Some(match best_price {
                None => n.price,
                Some(bp) => if self.side == Side::Bid { bp.max(n.price) } else { bp.min(n.price) },
            });
        }
        let price = best_price?;
        // collect candidates at that price
        let mut cand: Vec<(u64, u32)> = self.nodes.iter().enumerate()
            .filter(|(_, n)| n.qty > 0 && n.price == price)
            .map(|(i, n)| (n.inserted_slot, i as u32))
            .collect();
        cand.sort_unstable_by_key(|(slot, _)| *slot);
        cand.first().map(|(_, idx)| *idx)
    }
}
