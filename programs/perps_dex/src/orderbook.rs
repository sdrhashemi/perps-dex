use crate::state::Side;
use anchor_lang::prelude::*;

#[repr(C)]
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Copy)]
pub struct SlabNode {
    pub prev: u32,
    pub next: u32,
    pub key: u128,
    pub price: u64,
    pub qty: u64,
    pub owner: Pubkey,
}

impl Default for SlabNode {
    fn default() -> Self {
        Self {
            prev: u32::MAX,
            next: u32::MAX,
            key: 0,
            price: 0,
            qty: 0,
            owner: Pubkey::default(),
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
        let mut nodes = vec![SlabNode::default(); capacity];
        for i in 0..capacity as u32 {
            nodes[i as usize].next = if i + 1 < capacity as u32 {
                i + 1
            } else {
                u32::MAX
            };
        }
        Slab {
            nodes,
            head: u32::MAX,
            free_head: 0,
        }
    }

    fn alloc_node(&mut self) -> Option<u32> {
        let idx = self.free_head;
        if idx == u32::MAX {
            return None;
        }
        self.free_head = self.nodes[idx as usize].next;
        Some(idx)
    }

    fn dealloc_node(&mut self, idx: u32) {
        self.nodes[idx as usize] = SlabNode::default();
        self.nodes[idx as usize].next = self.free_head;
        self.free_head = idx;
    }

    pub fn insert(
        &mut self,
        key: u128,
        price: u64,
        qty: u64,
        owner: Pubkey,
        side: Side,
    ) -> Result<u32> {
        let idx = self
            .alloc_node()
            .ok_or_else(|| error!(crate::errors::ErrorCode::OrderbookOverflow))?;

        if self.head == u32::MAX {
            self.head = idx;
            let node = &mut self.nodes[idx as usize];
            *node = SlabNode {
                prev: u32::MAX,
                next: u32::MAX,
                key,
                price,
                qty,
                owner,
            };
            return Ok(idx);
        }

        let mut cur = self.head;
        let mut position: (u32, u32) = (u32::MAX, self.head);
        loop {
            let cur_node = &self.nodes[cur as usize];
            let better = match side {
                Side::Bid => price > cur_node.price,
                Side::Ask => price < cur_node.price,
            };
            if better || (price == cur_node.price && key < cur_node.key) {
                position = (cur_node.prev, cur);
                break;
            }
            if cur_node.next == u32::MAX {
                position = (cur, u32::MAX);
                break;
            }
            cur = cur_node.next;
        }

        let (prev, next) = position;
        {
            let node = &mut self.nodes[idx as usize];
            node.prev = prev;
            node.next = next;
            node.key = key;
            node.price = price;
            node.qty = qty;
            node.owner = owner;
        }
        if prev != u32::MAX {
            self.nodes[prev as usize].next = idx;
        } else {
            self.head = idx;
        }
        if next != u32::MAX {
            self.nodes[next as usize].prev = idx;
        }
        Ok(idx)
    }

    pub fn find_best(&self) -> Option<u32> {
        if self.head == u32::MAX {
            None
        } else {
            Some(self.head)
        }
    }

    pub fn reduce_order(&mut self, idx: u32, fill_qty: u64) -> Result<()> {
        if self.nodes[idx as usize].qty > fill_qty {
            self.nodes[idx as usize].qty = self.nodes[idx as usize].qty.saturating_sub(fill_qty);
        } else {
            self.remove(idx)?;
        }
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
