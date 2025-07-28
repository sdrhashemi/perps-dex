use std::u32;

use crate::errors::ErrorCode;
use crate::state::Side;
use anchor_lang::prelude::*;

#[repr(C)]
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Copy)]
pub struct SlabNode {
    pub key: u128,
    pub price: u64,
    pub qty: u64,
    pub owner: Pubkey,
    pub inserted_slot: u64,
    pub prev: Option<u32>,
    pub next: Option<u32>,
}

impl Default for SlabNode {
    fn default() -> Self {
        SlabNode {
            key: 0,
            price: 0,
            qty: 0,
            owner: Pubkey::default(),
            inserted_slot: 0,
            prev: None,
            next: None,
        }
    }
}

pub struct Slab {
    pub nodes: Vec<SlabNode>,
    pub head: u32,
    pub free_head: u32,
}

impl Slab {
    pub fn new(capacity: usize) -> Self {
        let mut nodes: Vec<SlabNode> = Vec::with_capacity(capacity);
        nodes.resize_with(capacity, || SlabNode::default());
        for i in 0..capacity - 1 {
            nodes[i].next = Some((i + 1) as u32);
        }
        nodes[capacity - 1].next = None;
        Self {
            nodes,
            head: u32::MAX,
            free_head: 0,
        }
    }

    fn allocate(&mut self) -> Result<u32> {
        let idx = self.free_head;
        if idx == u32::MAX {
            return Err(error!(ErrorCode::OrderbookOverflow));
        }
        let next_free = self.nodes[idx as usize]
            .next
            .ok_or(error!(ErrorCode::OrderbookOverflow))?;
        self.free_head = next_free;
        Ok(idx)
    }

    pub fn insert(&mut self, key: u128, price: u64, qty: u64, owner: Pubkey) -> Result<()> {
        // 1) Stamp current slot
        let slot = Clock::get()?.slot;
        // 2) Allocate a node
        let idx = self.allocate()?;
        let node = &mut self.nodes[idx as usize];
        node.key = key;
        node.price = price;
        node.qty = qty;
        node.owner = owner;
        node.inserted_slot = slot;
        node.next = None;
        node.prev = None;
        // 3) Link into price-level list
        self.link_node(idx)
    }

    pub fn find_best(&self) -> Option<u32> {
        let best_price = if self.is_bid_side() {
            self.nodes
                .iter()
                .filter(|n| n.qty > 0)
                .map(|n| n.price)
                .max()?;
        } else {
            self.nodes
                .iter()
                .filter(|n| n.qty > 0)
                .map(|n| n.price)
                .min()?;
        };

        let mut candidates: Vec<(u64, u32)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.price == best_price && n.qty > 0)
            .map(|(i, n)| (n.inserted_slot, i as u32))
            .collect();

        candidates.sort_unstable_by_key(|(slot, _)| *slot);
        candidates.first().map(|(_, idx)| *idx)
    }

    pub fn is_bid_side(&self) -> bool {
        true
    }
    
    pub fn reduce_order(&mut self, idx: u32, qty: u64) -> Result<()> {
        let node = &mut self.nodes[idx as usize];
        if qty >= node.qty {
            // remove from linked list
            if let Some(prev) = node.prev {
                self.nodes[prev as usize].next = node.next;
            } else {
                self.head = node.next.unwrap_or(u32::MAX);
            }
            if let Some(next) = node.next {
                self.nodes[next as usize].prev = node.prev;
            }
            // free slot
            node.next = Some(self.free_head);
            self.free_head = idx;
        } else {
            node.qty = node.qty.saturating_sub(qty);
        }
        Ok(())
    }

    fn link_node(&mut self, idx: u32) -> Result<()> {
        if self.head == u32::MAX {
            self.head = idx;
            return Ok(());
        }
        self.nodes[idx as usize].next = Some(self.head);
        self.nodes[self.head as usize].prev = Some(idx);
        self.head = idx;
        Ok(())
    }

    pub fn remove(&mut self, idx: u32) -> Result<()> {
        let (prev, next) = {
            let node = &self.nodes[idx as usize];
            (node.prev, node.next)
        };
        if prev != u32::MAX {
            self.nodes[prev as usize].next = next;
        } else {
            self.head = next;
        }
        if next != u32::MAX {
            self.nodes[next as usize].prev = prev;
        }
        self.dealloc_node(idx);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::pubkey::Pubkey;
    fn sample_pubkey(seed: u8) -> Pubkey {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        Pubkey::new_from_array(bytes)
    }

    #[test]
    fn test_insert_and_find_best_bid() {
        let mut slab = Slab::new(4);
        let pk1 = sample_pubkey(1);
        let pk2 = sample_pubkey(2);

        let idx1 = slab.insert(1, 100, 10, pk1, Side::Bid).unwrap();
        let idx2 = slab.insert(2, 110, 5, pk2, Side::Bid).unwrap();
        assert_eq!(slab.find_best(), Some(idx2));
        slab.reduce_order(idx2, 2).unwrap();
        assert_eq!(slab.nodes[idx2 as usize].qty, 3);

        slab.reduce_order(idx2, 3).unwrap();
        assert!(slab.find_best() == Some(idx1));
    }

    #[test]
    fn test_insert_ask_and_ordering() {
        let mut slab = Slab::new(3);
        let pk1 = sample_pubkey(3);
        let pk2 = sample_pubkey(4);

        let idx1 = slab.insert(1, 200, 10, pk1, Side::Ask).unwrap();
        let idx2 = slab.insert(2, 190, 8, pk2, Side::Ask).unwrap();
        assert_eq!(slab.find_best(), Some(idx2));
    }

    #[test]
    fn test_remove_from_middle() {
        let mut slab = Slab::new(5);
        let pk = sample_pubkey(5);

        let idx1 = slab.insert(1, 100, 1, pk, Side::Bid).unwrap();
        let idx2 = slab.insert(2, 90, 1, pk, Side::Bid).unwrap();
        let idx3 = slab.insert(3, 80, 1, pk, Side::Bid).unwrap();

        slab.remove(idx2).unwrap();

        assert_eq!(slab.head, idx1);
        assert_eq!(slab.nodes[idx1 as usize].next, idx3);
        assert_eq!(slab.nodes[idx3 as usize].prev, idx1);
    }
}
