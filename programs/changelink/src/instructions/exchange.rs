use crate::error::ChangeLinkError;
use crate::state::{Kiosk, Receipt};
use crate::{BPS_DENOMINATOR, KIOSK_SEED, RECEIPT_SEED};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

#[derive(Accounts)]
#[instruction(serial: u64)]
pub struct Exchange<'info> {
    /// The machine's operator key — the only signer allowed to settle a swap from this kiosk.
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [KIOSK_SEED, &kiosk.kiosk_id.to_le_bytes()],
        bump = kiosk.bump,
        has_one = authority @ ChangeLinkError::NotKioskAuthority,
    )]
    pub kiosk: Account<'info, Kiosk>,

    /// Source of every payout. Owned by the kiosk PDA, so the program signs for it.
    #[account(mut, address = kiosk.treasury @ ChangeLinkError::TreasuryMismatch)]
    pub treasury: Account<'info, TokenAccount>,

    #[account(address = kiosk.accepted_mint @ ChangeLinkError::MintMismatch)]
    pub mint: Account<'info, Mint>,

    /// The traveller's own token account. The stablecoin lands here and stops here — the
    /// machine takes no custody and holds no key to it.
    #[account(
        mut,
        constraint = destination.key() != treasury.key() @ ChangeLinkError::DestinationIsTreasury,
        constraint = destination.mint == kiosk.accepted_mint @ ChangeLinkError::MintMismatch,
    )]
    pub destination: Account<'info, TokenAccount>,

    /// One receipt per `(kiosk, serial)`. `init` fails on an existing PDA, which is the replay
    /// guard: the same note intake cannot be settled twice.
    #[account(
        init,
        payer = authority,
        space = 8 + Receipt::INIT_SPACE,
        seeds = [RECEIPT_SEED, kiosk.key().as_ref(), &serial.to_le_bytes()],
        bump
    )]
    pub receipt: Account<'info, Receipt>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn settle_exchange(
    ctx: Context<Exchange>,
    serial: u64,
    gross: u64,
    currency: [u8; 3],
    rate_micro: u64,
) -> Result<()> {
    let kiosk = &mut ctx.accounts.kiosk;

    require!(gross > 0, ChangeLinkError::ZeroAmount);

    // The "small amounts only" rule, enforced where an operator cannot bend it per customer,
    // and checked BEFORE any token movement.
    require!(
        gross <= kiosk.max_per_tx,
        ChangeLinkError::ExceedsPerTxLimit
    );
    require!(
        serial > kiosk.last_serial,
        ChangeLinkError::SerialNotMonotonic
    );

    let fee = ((gross as u128) * (kiosk.fee_bps as u128) / (BPS_DENOMINATOR as u128)) as u64;
    let net = gross
        .checked_sub(fee)
        .ok_or(ChangeLinkError::MathOverflow)?;
    require!(net > 0, ChangeLinkError::ZeroAmount);

    // Pay the traveller. The kiosk PDA is the treasury's owner, so it signs the CPI itself.
    let kiosk_id_bytes = kiosk.kiosk_id.to_le_bytes();
    let signer_seeds: &[&[u8]] = &[KIOSK_SEED, &kiosk_id_bytes, &[kiosk.bump]];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.treasury.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: kiosk.to_account_info(),
            },
            &[signer_seeds],
        ),
        net,
    )?;

    let receipt = &mut ctx.accounts.receipt;
    receipt.kiosk = kiosk.key();
    receipt.serial = serial;
    receipt.destination = ctx.accounts.destination.key();
    receipt.gross = gross;
    receipt.net = net;
    receipt.fee = fee;
    receipt.currency = currency;
    receipt.rate_micro = rate_micro;
    receipt.settled_at = Clock::get()?.unix_timestamp;
    receipt.bump = ctx.bumps.receipt;

    kiosk.last_serial = serial;
    kiosk.total_swaps = kiosk
        .total_swaps
        .checked_add(1)
        .ok_or(ChangeLinkError::MathOverflow)?;
    kiosk.total_gross = kiosk
        .total_gross
        .checked_add(gross)
        .ok_or(ChangeLinkError::MathOverflow)?;

    Ok(())
}
