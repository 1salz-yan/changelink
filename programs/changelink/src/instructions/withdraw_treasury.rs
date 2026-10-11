use crate::error::ChangeLinkError;
use crate::state::Kiosk;
use crate::KIOSK_SEED;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

#[derive(Accounts)]
pub struct WithdrawTreasury<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [KIOSK_SEED, &kiosk.kiosk_id.to_le_bytes()],
        bump = kiosk.bump,
        has_one = authority @ ChangeLinkError::NotKioskAuthority,
    )]
    pub kiosk: Account<'info, Kiosk>,

    #[account(mut, address = kiosk.treasury @ ChangeLinkError::TreasuryMismatch)]
    pub treasury: Account<'info, TokenAccount>,

    /// The operator's own account. Sweeping floats out is what turns volume into revenue.
    #[account(
        mut,
        constraint = destination.key() != treasury.key() @ ChangeLinkError::DestinationIsTreasury,
        constraint = destination.mint == kiosk.accepted_mint @ ChangeLinkError::MintMismatch,
    )]
    pub destination: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn sweep_treasury(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
    require!(amount > 0, ChangeLinkError::ZeroAmount);

    let kiosk = &ctx.accounts.kiosk;
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
        amount,
    )?;

    Ok(())
}
