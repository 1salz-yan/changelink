pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("4GA9eg49dVH8hpJmN7dkiodXNcAUuyi9wiXjeGkeUVCF");

#[program]
pub mod changelink {
    use super::*;

    /// Stand up one machine. `kiosk_id` is the machine serial number and the PDA seed, so a
    /// fleet is a set of independent config accounts under this one program rather than a
    /// shared mutable record.
    pub fn initialize_kiosk(
        ctx: Context<InitializeKiosk>,
        kiosk_id: u64,
        fee_bps: u16,
        max_per_tx: u64,
    ) -> Result<()> {
        initialize_kiosk::init_kiosk(ctx, kiosk_id, fee_bps, max_per_tx)
    }

    /// Settle one exchange. The machine pays the traveller's own token account and writes a
    /// receipt. This is the whole product on-chain: value leaves the machine's treasury and
    /// lands in a wallet the operator does not control.
    pub fn exchange(
        ctx: Context<Exchange>,
        serial: u64,
        gross: u64,
        currency: [u8; 3],
        rate_micro: u64,
    ) -> Result<()> {
        exchange::settle_exchange(ctx, serial, gross, currency, rate_micro)
    }

    /// Operator-only: change the fee and the per-transaction ceiling.
    pub fn set_limits(ctx: Context<SetLimits>, fee_bps: u16, max_per_tx: u64) -> Result<()> {
        set_limits::apply_limits(ctx, fee_bps, max_per_tx)
    }

    /// Operator-only: sweep collected fees out of the kiosk treasury.
    pub fn withdraw_treasury(ctx: Context<WithdrawTreasury>, amount: u64) -> Result<()> {
        withdraw_treasury::sweep_treasury(ctx, amount)
    }

    /// Traveller-signed: send stablecoin back into the machine and take notes out. The other
    /// half of the machine — cash in, and cash out.
    pub fn redeem(
        ctx: Context<Redeem>,
        serial: u64,
        amount: u64,
        currency: [u8; 3],
        rate_micro: u64,
    ) -> Result<()> {
        redeem::cash_out(ctx, serial, amount, currency, rate_micro)
    }

    /// Operator-only: record a physical count of the cash box. A box that comes up short
    /// freezes the machine; a count that adds up unfreezes it.
    pub fn attest_backing(ctx: Context<AttestBacking>, counted: u64, removed: u64) -> Result<()> {
        attest_backing::record_backing(ctx, counted, removed)
    }
}
