use anchor_lang::prelude::*;

/// One receipt per settled exchange, keyed by `(kiosk, serial)`.
///
/// Its existence is the replay guard: an `init` on an existing PDA fails, so the same note
/// intake cannot be settled twice. It is also the audit trail — where the money went, what
/// the traveller handed over, what they got, and when.
#[account]
#[derive(InitSpace)]
pub struct Receipt {
    pub kiosk: Pubkey,

    /// The machine's own note counter for this swap.
    pub serial: u64,

    /// The traveller's token account — where the stablecoin actually landed. This is the
    /// field a customer dispute is settled with, and the reason the machine keeps a record
    /// at all even though it never custodies the money.
    pub destination: Pubkey,

    /// Tendered value in base units, as reported by the note acceptor.
    pub gross: u64,

    /// What the traveller received: `gross - fee`.
    pub net: u64,

    pub fee: u64,

    /// ISO-4217 code of the tendered cash, e.g. `EUR`. ASCII, 3 bytes.
    pub currency: [u8; 3],

    /// Rate applied, scaled by 1e6 (so 1.0873 is stored as 1_087_300).
    pub rate_micro: u64,

    /// Unix timestamp of settlement.
    pub settled_at: i64,

    pub bump: u8,
}
