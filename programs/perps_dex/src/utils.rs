use crate::{instruction::*, state::*};
use anchor_lang::prelude::*;

pub fn initialize_market(
    ctx: Context<InitializeMarket>,
    market_nonce: u8,
    params: MarketParams,
) -> Result<()> {
    let m = &mut ctx.accounts.market;
}
