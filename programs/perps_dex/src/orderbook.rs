use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::state::Side;

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Default)]
pub struct SlabNode {
    pub key: u128,
    pub price: u64,
    pub qty: u64,
    pub owner: Pubkey,
    pub inserted_slot: u64,
    pub next: Option<u32>,
    pub prev: Option<u32>,
}

pub struct Slab {
    pub nodes: Vec<SlabNode>,
    pub head: Option<u32>,
    pub free_head: Option<u32>,
    pub side: Side,
}

impl Slab {
    pub fn new(capacity: usize, side: Side) -> Self {
        let mut nodes = Vec::with_capacity(capacity);
        nodes.resize_with(capacity, || SlabNode::default());
        for i in 0..capacity - 1 {
            nodes[i].next = Some((i + 1) as u32);
        }
        nodes[capacity - 1].next = None;
        Self { nodes, head: None, free_head: Some(0), side }
    }

    fn allocate(&mut self) -> Result<u32> {
        let idx = self.free_head.ok_or(error!(ErrorCode::OrderbookOverflow))?;
        self.free_head = self.nodes[idx as usize].next;
        Ok(idx)
    }

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

    pub fn is_bid_side(&self) -> bool {
        self.side == Side::Bid
    }

    pub fn find_best(&self) -> Option<u32> {
        let mut best_price: Option<u64> = None;
        for n in &self.nodes {
            if n.qty == 0 { continue; }
            best_price = Some(match best_price {
                None => n.price,
                Some(bp) => if self.side == Side::Bid { bp.max(n.price) } else { bp.min(n.price) },
            });
        }
        let price = best_price?;
        let mut cand: Vec<(u64, u32)> = self.nodes.iter().enumerate()
            .filter(|(_, n)| n.qty > 0 && n.price == price)
            .map(|(i, n)| (n.inserted_slot, i as u32))
            .collect();
        cand.sort_unstable_by_key(|(slot, _)| *slot);
        cand.first().map(|(_, idx)| *idx)
    }
}
