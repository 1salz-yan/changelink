use crate::error::ChangeLinkError;
use crate::state::Kiosk;
use crate::KIOSK_SEED;
use anchor_lang::prelude::*;

/// Operator-only: record a physical count of the cash box.
///
/// This is the machine's honesty check, and it is deliberately the *mechanism* rather than
/// an identity: the operator says what the box actually holds and what they took out, the
/// program compares it with what the chain says was taken in, and if the box is short the
/// machine freezes itself. A short count is not an error — being frozen is the outcome, and
/// re-attesting a good count is what unfreezes it.
#[derive(Accounts)]
pub struct AttestBacking<'info> {
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [KIOSK_SEED, &kiosk.kiosk_id.to_le_bytes()],
        bump = kiosk.bump,
        has_one = authority @ ChangeLinkError::NotKioskAuthority,
    )]
    pub kiosk: Account<'info, Kiosk>,
}

pub fn record_backing(ctx: Context<AttestBacking>, counted: u64, removed: u64) -> Result<()> {
    let kiosk = &mut ctx.accounts.kiosk;

    // What the chain says should be in the box: everything taken in, less everything the
    // operator has previously declared taking out, less everything the machine has already
    // handed back out through `redeem`.
    kiosk.total_removed = kiosk
        .total_removed
        .checked_add(removed)
        .ok_or(ChangeLinkError::MathOverflow)?;
    let expected = kiosk
        .total_gross
        .saturating_sub(kiosk.total_removed)
        .saturating_sub(kiosk.total_redeemed);

    // Notes are countable, so the tolerance exists only for the odd rounding of a
    // foreign-currency conversion, not to excuse a gap.
    let slack = (expected / 10_000 * crate::BACKING_TOLERANCE_BPS).max(1);
    kiosk.counted_backing = counted;
    kiosk.backing_checked_at = Clock::get()?.unix_timestamp;
    kiosk.frozen = counted.saturating_add(slack) < expected;

    Ok(())
}
