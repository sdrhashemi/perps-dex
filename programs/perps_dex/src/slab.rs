use crate::errors::ErrorCode;
use crate::state::Side;
use anchor_lang::prelude::*;

pub const NULL_INDEX: u32 = u32::MAX;
pub const MAX_SLAB_CAPACITY: usize = 140;

/// Zero-copy slab node for in-place mutation\認

#[account(zero_copy)]
#[repr(C)]
pub struct SlabNode {
    pub key: u128,          // 16 bytes
    pub price: u64,         // 8 bytes
    pub qty: u64,           // 8 bytes
    pub owner: Pubkey,      // 32 bytes
    pub inserted_slot: u64, // 8 bytes
    pub next: u32,          // index or NULL_INDEX
    pub prev: u32,          // index or NULL_INDEX
}

/// Zero-copy slab structure stored on-chain\認

#[account(zero_copy)]
#[repr(C)]
pub struct Slab {
    pub head: u32,                            // 4 bytes
    pub free_head: u32,                       // 4 bytes
    pub side: u8,                             // 1 byte: 0 = Bid, 1 = Ask
    pub _padding: [u8; 7],                    // pad to 16-byte alignment
    pub nodes: [SlabNode; MAX_SLAB_CAPACITY], // fixed array of nodes
}

impl Slab {
    /// Initialize the free list and side
    pub fn init(&mut self, capacity: usize, side: u8) -> Result<()> {
        require!(
            capacity > 0 && capacity <= MAX_SLAB_CAPACITY,
            ErrorCode::InvalidOrderbookCapacity
        );
        for i in 0..capacity {
            self.nodes[i].next = if i + 1 < capacity {
                (i + 1) as u32
            } else {
                NULL_INDEX
            };
            self.nodes[i].prev = NULL_INDEX;
        }
        self.head = NULL_INDEX;
        self.free_head = 0;
        self.side = side;
        Ok(())
    }

    /// Insert a new order in sorted order, mutating in-place
    pub fn insert(
        &mut self,
        key: u128,
        price: u64,
        qty: u64,
        owner: Pubkey,
        slot: u64,
    ) -> Result<u32> {
        require!(qty > 0, ErrorCode::InvalidQuantity);
        let idx = self.free_head;
        require!(idx != NULL_INDEX, ErrorCode::OrderbookFull);
        let i = idx as usize;
        self.free_head = self.nodes[i].next;

        let mut curr = self.head;
        let mut prev = NULL_INDEX;
        while curr != NULL_INDEX {
            let node = &self.nodes[curr as usize];
            let should = if self.side == Side::Bid as u8 {
                price > node.price || (price == node.price && slot < node.inserted_slot)
            } else {
                price < node.price || (price == node.price && slot < node.inserted_slot)
            };
            if should {
                break;
            }
            prev = curr;
            curr = node.next;
        }

        let node = &mut self.nodes[i];
        node.key = key;
        node.price = price;
        node.qty = qty;
        node.owner = owner;
        node.inserted_slot = slot;
        node.prev = prev;
        node.next = curr;

        if prev != NULL_INDEX {
            self.nodes[prev as usize].next = idx;
        } else {
            self.head = idx;
        }
        if curr != NULL_INDEX {
            self.nodes[curr as usize].prev = idx;
        }
        Ok(idx)
    }

    /// Remove a node by index, unlink and free
    pub fn remove(&mut self, idx: u32) -> Result<()> {
        let i = idx as usize;
        require!(i < MAX_SLAB_CAPACITY, ErrorCode::InvalidIndex);
        let prev = self.nodes[i].prev;
        let next = self.nodes[i].next;
        if prev != NULL_INDEX {
            self.nodes[prev as usize].next = next;
        } else {
            self.head = next;
        }
        if next != NULL_INDEX {
            self.nodes[next as usize].prev = prev;
        }
        let node = &mut self.nodes[i];
        node.key = 0;
        node.price = 0;
        node.qty = 0;
        node.owner = Pubkey::default();
        node.inserted_slot = 0;
        node.prev = NULL_INDEX;
        // prepend to free list
        node.next = self.free_head;
        self.free_head = idx;
        Ok(())
    }

    /// Return index of best active order
    pub fn best(&self) -> Option<u32> {
        if self.head == NULL_INDEX {
            None
        } else {
            Some(self.head)
        }
    }
}
