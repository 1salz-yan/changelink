use crate::error::ChangeLinkError;
use crate::state::Kiosk;
use crate::{KIOSK_SEED, MAX_FEE_BPS};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SetLimits<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [KIOSK_SEED, &kiosk.kiosk_id.to_le_bytes()],
        bump = kiosk.bump,
        has_one = authority @ ChangeLinkError::NotKioskAuthority,
    )]
    pub kiosk: Account<'info, Kiosk>,
}

pub fn apply_limits(ctx: Context<SetLimits>, fee_bps: u16, max_per_tx: u64) -> Result<()> {
    // The program's own ceiling applies here too, so raising the fee is bounded by code rather
    // than by the operator's good behaviour.
    require!(fee_bps <= MAX_FEE_BPS, ChangeLinkError::FeeTooHigh);
    require!(max_per_tx > 0, ChangeLinkError::ZeroLimit);

    let kiosk = &mut ctx.accounts.kiosk;
    kiosk.fee_bps = fee_bps;
    kiosk.max_per_tx = max_per_tx;

    Ok(())
}
