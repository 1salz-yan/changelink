use anchor_lang::prelude::*;

/// Per-machine configuration. One PDA per physical kiosk, seeded by the machine's serial
/// number, so a fleet is a set of independent PDAs under one program.
#[account]
#[derive(InitSpace)]
pub struct Kiosk {
    /// The machine operator's key. Only this key may settle exchanges from this kiosk.
    pub authority: Pubkey,

    /// Machine serial number. Seeds the PDA, so it is immutable once the machine is up.
    pub kiosk_id: u64,

    /// The stablecoin this machine settles in.
    pub accepted_mint: Pubkey,

    /// The operator's payout account. Owned by the kiosk PDA (checked at init), which is why
    /// the program can move funds out of it and the operator cannot simply drain it with a
    /// wallet signature.
    pub treasury: Pubkey,

    /// Exchange fee in basis points. Capped at `MAX_FEE_BPS` at initialisation and at every
    /// later change.
    pub fee_bps: u16,

    /// Hard per-transaction ceiling in stablecoin base units. This is the product's
    /// "small amounts only" rule, and it lives here — on-chain — so that no operator (and no
    /// stolen operator key) can quietly raise it per customer.
    pub max_per_tx: u64,

    /// Number of settled exchanges. Part of the machine's public audit trail.
    pub total_swaps: u64,

    /// Gross tendered volume, base units.
    pub total_gross: u64,

    /// Highest receipt serial settled so far. Receipt PDAs are the primary replay guard;
    /// this is the second one, and it is cheap.
    pub last_serial: u64,

    // ---- cash-box reconciliation.
    //
    // What the machine can honestly claim is *not* "supply equals reserve" — with the value
    // gone to the traveller's own wallet we hold no claim to back. What the chain can hold
    // the operator to is the cash side: everything that went in, less everything they have
    // declared taking out, should still be in the box.
    /// Declared physical removals from the cash box, base units.
    pub total_removed: u64,

    /// Cash handed back out through `redeem`, in the same bookkeeping unit. Cash leaving the
    /// box is not a discrepancy — but the chain knows about it, so the count must account for
    /// it just the same.
    pub total_redeemed: u64,

    /// The operator's most recent physical count, base units.
    pub counted_backing: u64,

    /// When that count was taken.
    pub backing_checked_at: i64,

    /// Set when a physical count came up short: the machine settles nothing until an
    /// operator attests a count that adds up. Stopping is the point — a machine that keeps
    /// taking notes while its box is short is the failure this exists to prevent.
    pub frozen: bool,

    pub bump: u8,
}
