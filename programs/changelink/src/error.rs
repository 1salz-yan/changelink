use anchor_lang::prelude::*;

#[error_code]
pub enum ChangeLinkError {
    #[msg("Fee exceeds the 10% ceiling baked into the program")]
    FeeTooHigh,

    #[msg("Per-transaction limit must be greater than zero")]
    ZeroLimit,

    #[msg("Amount exceeds this kiosk's per-transaction limit")]
    ExceedsPerTxLimit,

    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("Receipt serial must increase monotonically for this kiosk")]
    SerialNotMonotonic,

    #[msg("Signer is not this kiosk's operator authority")]
    NotKioskAuthority,

    #[msg("Treasury account does not match the one recorded for this kiosk")]
    TreasuryMismatch,

    #[msg("Treasury account must be owned by the kiosk PDA, so the program can sign for it")]
    TreasuryNotOwnedByKiosk,

    #[msg("Token account mint does not match the stablecoin this kiosk settles in")]
    MintMismatch,

    #[msg("Destination must not be the kiosk treasury itself")]
    DestinationIsTreasury,

    #[msg("This machine is frozen: a physical count of its cash box came up short")]
    MachineFrozen,

    #[msg("Cash out is capped at a quarter of this kiosk's per-transaction limit")]
    ExceedsRedeemLimit,

    #[msg("Arithmetic overflow")]
    MathOverflow,
}
