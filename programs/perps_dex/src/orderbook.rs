use crate::errors::ErrorCode;
use crate::state::Side;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
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
    pub fn new(capacity: usize, side: u8) -> Result<Self> {
        require!(
            capacity > 0 && capacity <= 10000,
            ErrorCode::InvalidSlabCapcity
        );
        let side_enum = match side {
            0 => Side::Bid,
            1 => Side::Ask,
            _ => return Err(error!(ErrorCode::InvalidOrderbookSide)),
        };
        let mut nodes = Vec::with_capacity(capacity);
        nodes.resize_with(capacity, || SlabNode::default());
        for i in 0..capacity - 1 {
            nodes[i].next = Some((i + 1) as u32);
        }
        nodes[capacity - 1].next = None;
        Ok(Self {
            nodes,
            head: None,
            free_head: Some(0),
            side: side_enum,
        })
    }

    fn allocate(&mut self) -> Result<u32> {
        let idx = self.free_head.ok_or(error!(ErrorCode::OrderbookOverflow))?;
        self.free_head = self.nodes[idx as usize].next;
        self.nodes[idx as usize] = SlabNode {
            next: None,
            prev: None,
            ..Default::default()
        };
        Ok(idx)
    }

    pub fn insert(
        &mut self,
        key: u128,
        price: u64,
        qty: u64,
        owner: Pubkey,
        slot: u64,
    ) -> Result<()> {
        require!(qty > 0, ErrorCode::InvalidQuantity);
        let idx = self.allocate()?;

        let mut current = self.head;
        let mut prev: Option<u32> = None;
        while let Some(curr_idx) = current {
            require!(curr_idx < self.nodes.len() as u32, ErrorCode::InvalidIndex);
            let curr_price = self.nodes[curr_idx as usize].price;
            let curr_slot = self.nodes[curr_idx as usize].inserted_slot;
            let should_insert = match self.side {
                Side::Bid => price > curr_price || (price == curr_price && slot < curr_slot),
                Side::Ask => price < curr_price || (price == curr_price && slot < curr_slot),
            };
            if should_insert {
                break;
            }
            prev = current;
            current = self.nodes[curr_idx as usize].next;
        }

        {
            let (left, right) = self.nodes.split_at_mut(idx as usize);
            let node = if idx == 0 {
                &mut left[0]
            } else {
                &mut right[0]
            };
            node.key = key;
            node.price = price;
            node.qty = qty;
            node.owner = owner;
            node.inserted_slot = slot;
            node.prev = prev;
            node.next = current;
        }

        if let Some(prev_idx) = prev {
            require!(prev_idx < self.nodes.len() as u32, ErrorCode::InvalidIndex);
            let (prev_left, prev_right) = self.nodes.split_at_mut(prev_idx as usize);
            let prev_node = if prev_idx == 0 {
                &mut prev_left[0]
            } else {
                &mut prev_right[0]
            };
            prev_node.next = Some(idx);
        } else {
            self.head = Some(idx);
        }

        if let Some(next_idx) = current {
            require!(next_idx < self.nodes.len() as u32, ErrorCode::InvalidIndex);
            let (next_left, next_right) = self.nodes.split_at_mut(next_idx as usize);
            let next_node = if next_idx == 0 {
                &mut next_left[0]
            } else {
                &mut next_right[0]
            };
            next_node.prev = Some(idx);
        }

        Ok(())
    }

    pub fn reduce_order(&mut self, idx: u32, qty: u64) -> Result<()> {
        require!(idx < self.nodes.len() as u32, ErrorCode::InvalidIndex);
        require!(qty > 0, ErrorCode::InvalidQuantity);
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
            self.nodes[idx as usize] = SlabNode {
                next: self.free_head,
                prev: None,
                ..Default::default()
            };
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
        let mut current = self.head;
        while let Some(idx) = current {
            if self.nodes[idx as usize].qty > 0 {
                return Some(idx);
            }
            current = self.nodes[idx as usize].next;
        }
        None
    }
}

pub fn decode_slab(
    slab: &[u8],
    head: Option<u32>,
    free_head: Option<u32>,
    side: Side,
) -> Result<Slab> {
    let node_size = std::mem::size_of::<SlabNode>();
    let capacity = slab.len() / node_size;
    require!(slab.len() % node_size == 0, ErrorCode::InvalidSlabData);
    let mut nodes = Vec::with_capacity(capacity);
    for i in 0..capacity {
        let start = i * node_size;
        let end = start + node_size;
        let node_data = &slab[start..end];
        let node = SlabNode::deserialize(&mut &node_data[..])?;
        nodes.push(node);
    }
    Ok(Slab {
        nodes,
        head: head.map(|h| h as u32),
        free_head: free_head.map(|f| f as u32),
        side,
    })
}

pub fn encode_slab(slab: &Slab) -> Result<(Vec<u8>, u32, u32)> {
    let mut bytes = Vec::with_capacity(slab.nodes.len() * std::mem::size_of::<SlabNode>());
    for node in &slab.nodes {
        node.serialize(&mut bytes)?;
    }
    Ok((bytes, slab.head.unwrap_or(0), slab.free_head.unwrap_or(0)))
}
