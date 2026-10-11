use crate::error::ChangeLinkError;
use crate::state::{Kiosk, Receipt, KIND_REDEEM};
use crate::{BPS_DENOMINATOR, KIOSK_SEED, RECEIPT_SEED};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

/// The other direction: the traveller sends stablecoin back into the machine's treasury and
/// the machine pays them notes.
///
/// The traveller signs here — their own token account is the source, so nothing moves without
/// their key. Cash out is capped far tighter than cash in (`max_per_tx / 4`): a machine that
/// runs out of notes mid-swap has taken the traveller's stablecoin for nothing, so the
/// conservative direction is to dispense less.
#[derive(Accounts)]
#[instruction(serial: u64)]
pub struct Redeem<'info> {
    /// The traveller. Their signature is what authorises the stablecoin to move.
    #[account(mut)]
    pub traveller: Signer<'info>,

    #[account(
        mut,
        seeds = [KIOSK_SEED, &kiosk.kiosk_id.to_le_bytes()],
        bump = kiosk.bump,
    )]
    pub kiosk: Account<'info, Kiosk>,

    /// The traveller's own token account. The Token program enforces that it really is theirs.
    #[account(
        mut,
        constraint = source.mint == kiosk.accepted_mint @ ChangeLinkError::MintMismatch,
    )]
    pub source: Account<'info, TokenAccount>,

    #[account(mut, address = kiosk.treasury @ ChangeLinkError::TreasuryMismatch)]
    pub treasury: Account<'info, TokenAccount>,

    #[account(address = kiosk.accepted_mint @ ChangeLinkError::MintMismatch)]
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = traveller,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [RECEIPT_SEED, kiosk.key().as_ref(), &serial.to_le_bytes()],
        bump
    )]
    pub receipt: Account<'info, Receipt>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn cash_out(
    ctx: Context<Redeem>,
    serial: u64,
    amount: u64,
    currency: [u8; 3],
    rate_micro: u64,
) -> Result<()> {
    let kiosk = &mut ctx.accounts.kiosk;

    require!(!kiosk.frozen, ChangeLinkError::MachineFrozen);
    require!(amount > 0, ChangeLinkError::ZeroAmount);
    require!(
        amount <= kiosk.max_per_tx / 4,
        ChangeLinkError::ExceedsRedeemLimit
    );
    require!(
        serial > kiosk.last_serial,
        ChangeLinkError::SerialNotMonotonic
    );

    let fee = ((amount as u128) * (kiosk.fee_bps as u128) / (BPS_DENOMINATOR as u128)) as u64;
    let payout = amount
        .checked_sub(fee)
        .ok_or(ChangeLinkError::MathOverflow)?;
    require!(payout > 0, ChangeLinkError::ZeroAmount);

    // The traveller signs for their own account: `token::transfer` with the traveller as
    // authority, no PDA signing involved on this side.
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.source.to_account_info(),
                to: ctx.accounts.treasury.to_account_info(),
                authority: ctx.accounts.traveller.to_account_info(),
            },
        ),
        amount,
    )?;

    let receipt = &mut ctx.accounts.receipt;
    receipt.kiosk = kiosk.key();
    receipt.serial = serial;
    receipt.destination = ctx.accounts.traveller.key();
    receipt.gross = amount;
    receipt.net = payout;
    receipt.fee = fee;
    receipt.currency = currency;
    receipt.rate_micro = rate_micro;
    receipt.settled_at = Clock::get()?.unix_timestamp;
    receipt.kind = KIND_REDEEM;
    receipt.bump = ctx.bumps.receipt;

    kiosk.last_serial = serial;
    kiosk.total_swaps = kiosk
        .total_swaps
        .checked_add(1)
        .ok_or(ChangeLinkError::MathOverflow)?;
    kiosk.total_redeemed = kiosk
        .total_redeemed
        .checked_add(payout)
        .ok_or(ChangeLinkError::MathOverflow)?;

    Ok(())
}
