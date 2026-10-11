use crate::error::ChangeLinkError;
use crate::state::Kiosk;
use crate::KIOSK_SEED;
use crate::MAX_FEE_BPS;
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

#[derive(Accounts)]
#[instruction(kiosk_id: u64)]
pub struct InitializeKiosk<'info> {
    /// The machine operator. Pays for the kiosk account and owns its configuration.
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + Kiosk::INIT_SPACE,
        seeds = [KIOSK_SEED, &kiosk_id.to_le_bytes()],
        bump
    )]
    pub kiosk: Account<'info, Kiosk>,

    /// The stablecoin this machine settles in, recorded once here.
    pub mint: Account<'info, Mint>,

    /// The machine's payout account. It must be owned by the kiosk PDA: that is what lets the
    /// program move money out of it on a settlement, and it is why the operator cannot point
    /// the machine at an account the program cannot sign for.
    #[account(
        constraint = treasury.owner == kiosk.key() @ ChangeLinkError::TreasuryNotOwnedByKiosk,
        constraint = treasury.mint == mint.key() @ ChangeLinkError::MintMismatch,
    )]
    pub treasury: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
}

pub fn init_kiosk(
    ctx: Context<InitializeKiosk>,
    kiosk_id: u64,
    fee_bps: u16,
    max_per_tx: u64,
) -> Result<()> {
    require!(fee_bps <= MAX_FEE_BPS, ChangeLinkError::FeeTooHigh);
    require!(max_per_tx > 0, ChangeLinkError::ZeroLimit);

    let kiosk = &mut ctx.accounts.kiosk;
    kiosk.authority = ctx.accounts.authority.key();
    kiosk.kiosk_id = kiosk_id;
    kiosk.accepted_mint = ctx.accounts.mint.key();
    kiosk.treasury = ctx.accounts.treasury.key();
    kiosk.fee_bps = fee_bps;
    kiosk.max_per_tx = max_per_tx;
    kiosk.total_swaps = 0;
    kiosk.total_gross = 0;
    kiosk.last_serial = 0;
    kiosk.bump = ctx.bumps.kiosk;

    Ok(())
}
