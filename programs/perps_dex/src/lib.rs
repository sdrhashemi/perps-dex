use anchor_lang::prelude::*;

declare_id!("7k59y4LUVtb9t9kYVKEkQnn7e8JW4BvowLbYsLawAoBs");

#[program]
pub mod perps_dex {
    use super::*;

    pub fn initialize_orderbook() -> Result<()> {

    }

    pub fn place_order() -> Result<()> {
        
    }
}

#[derive(Accounts)]
pub struct Initialize {}
