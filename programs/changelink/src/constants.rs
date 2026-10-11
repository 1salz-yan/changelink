use anchor_lang::prelude::*;

/// Seeds for the per-machine config PDA: `[b"kiosk", kiosk_id.to_le_bytes()]`.
#[constant]
pub const KIOSK_SEED: &[u8] = b"kiosk";

/// Seeds for a settlement receipt: `[b"receipt", kiosk, serial.to_le_bytes()]`.
#[constant]
pub const RECEIPT_SEED: &[u8] = b"receipt";

/// Basis-point denominator. 1 bps = 0.01%.
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Absolute fee ceiling baked into the program. No operator, and no compromised operator
/// key, can make a machine charge more than 10% of the tendered amount.
pub const MAX_FEE_BPS: u16 = 1_000;

/// How far a physical cash-box count may sit below the chain's record before the machine
/// freezes itself. 100 bps = 1%: enough to absorb the rounding of a foreign-currency
/// conversion, not enough to hide a missing bundle of notes.
pub const BACKING_TOLERANCE_BPS: u64 = 100;
