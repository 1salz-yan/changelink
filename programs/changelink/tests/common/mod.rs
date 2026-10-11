#![allow(dead_code)]

//! Shared harness for the ChangeLink tests.
//!
//! Everything here runs against a real SPL Token program inside LiteSVM, with real
//! token transfers. Banknotes are the only thing simulated — the money movement is not.

use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{
            instruction::{AccountMeta, Instruction},
            system_program,
        },
        InstructionData, ToAccountMetas,
    },
    litesvm::{
        types::{FailedTransactionMetadata, TransactionMetadata},
        LiteSVM,
    },
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    std::str::FromStr,
};

pub const KIOSK_SEED: &[u8] = b"kiosk";
pub const RECEIPT_SEED: &[u8] = b"receipt";

pub const ONE_SOL: u64 = 1_000_000_000;

/// Base units for the demo stablecoin. 6 decimals, like USDC.
pub const ONE_USDC: u64 = 1_000_000;

/// Size of an SPL mint / token account, per the Token program's own layout.
pub const MINT_LEN: usize = 82;
pub const TOKEN_ACCOUNT_LEN: usize = 165;

pub fn token_program_id() -> Pubkey {
    Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap()
}

pub fn setup_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();
    // `LiteSVM::new()` already loads the native SPL Token / ATA / Memo programs, so a
    // `token::transfer` CPI executes for real here.
    svm.add_program(
        changelink::id(),
        include_bytes!("../../../../target/deploy/changelink.so"),
    )
    .unwrap();
    svm
}

pub fn fund(svm: &mut LiteSVM, pubkey: &Pubkey, lamports: u64) {
    svm.airdrop(pubkey, lamports).unwrap();
}

pub fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    instructions: &[Instruction],
    extra_signers: &[&Keypair],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    // LiteSVM's blockhash is static, so two byte-identical attempts would hash to the same
    // signature and the second would come back as `AlreadyProcessed` rather than as the
    // program's own error. Expiring the blockhash first gives every attempt a fresh one, the
    // way a real cluster would.
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(instructions, Some(&payer.pubkey()), &blockhash);

    let mut signers: Vec<&Keypair> = Vec::with_capacity(1 + extra_signers.len());
    signers.push(payer);
    signers.extend_from_slice(extra_signers);

    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
}

// ---------------------------------------------------------------- PDAs

pub fn kiosk_pda(kiosk_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[KIOSK_SEED, &kiosk_id.to_le_bytes()], &changelink::id())
}

pub fn receipt_pda(kiosk: &Pubkey, serial: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[RECEIPT_SEED, kiosk.as_ref(), &serial.to_le_bytes()],
        &changelink::id(),
    )
}

// ------------------------------------------------- system + SPL plumbing
//
// The instructions below are assembled by hand (byte layouts straight out of the System
// program and the Token program) so the harness needs no extra client crates. The token
// program they call is the real one.

/// System `CreateAccount` (instruction index 0).
pub fn create_account_ix(
    from: &Pubkey,
    to: &Pubkey,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(52);
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    data.extend_from_slice(&space.to_le_bytes());
    data.extend_from_slice(owner.as_ref());

    Instruction::new_with_bytes(
        system_program::ID,
        &data,
        vec![AccountMeta::new(*from, true), AccountMeta::new(*to, true)],
    )
}

/// SPL Token `InitializeMint2` (tag 20): decimals + mint authority, no freeze authority.
pub fn spl_initialize_mint2_ix(mint: &Pubkey, decimals: u8, authority: &Pubkey) -> Instruction {
    let mut data = Vec::with_capacity(38);
    data.push(20u8);
    data.push(decimals);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&0u32.to_le_bytes()); // COption::None — no freeze authority

    Instruction::new_with_bytes(
        token_program_id(),
        &data,
        vec![AccountMeta::new(*mint, false)],
    )
}

/// SPL Token `InitializeAccount3` (tag 18): sets the owner; no rent sysvar required.
pub fn spl_initialize_account3_ix(account: &Pubkey, mint: &Pubkey, owner: &Pubkey) -> Instruction {
    let mut data = Vec::with_capacity(33);
    data.push(18u8);
    data.extend_from_slice(owner.as_ref());

    Instruction::new_with_bytes(
        token_program_id(),
        &data,
        vec![
            AccountMeta::new(*account, false),
            AccountMeta::new_readonly(*mint, false),
        ],
    )
}

/// SPL Token `MintTo` (tag 7).
pub fn spl_mint_to_ix(mint: &Pubkey, to: &Pubkey, authority: &Pubkey, amount: u64) -> Instruction {
    let mut data = Vec::with_capacity(9);
    data.push(7u8);
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction::new_with_bytes(
        token_program_id(),
        &data,
        vec![
            AccountMeta::new(*mint, false),
            AccountMeta::new(*to, false),
            AccountMeta::new_readonly(*authority, true),
        ],
    )
}

/// SPL Token account balance. Layout: mint(32) owner(32) amount(8) …
pub fn token_amount(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let acc = svm
        .get_account(account)
        .expect("token account should exist");
    u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
}

pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, decimals: u8) -> Keypair {
    let mint = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(MINT_LEN);
    let create = create_account_ix(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        MINT_LEN as u64,
        &token_program_id(),
    );
    send(svm, payer, &[create], &[&mint]).expect("create mint account");

    let init = spl_initialize_mint2_ix(&mint.pubkey(), decimals, &payer.pubkey());
    send(svm, payer, &[init], &[]).expect("initialize mint");

    mint
}

pub fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Keypair {
    let account = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(TOKEN_ACCOUNT_LEN);
    let create = create_account_ix(
        &payer.pubkey(),
        &account.pubkey(),
        rent,
        TOKEN_ACCOUNT_LEN as u64,
        &token_program_id(),
    );
    send(svm, payer, &[create], &[&account]).expect("create token account");

    let init = spl_initialize_account3_ix(&account.pubkey(), mint, owner);
    send(svm, payer, &[init], &[]).expect("initialize token account");

    account
}

// ------------------------------------------------- ChangeLink instructions

pub fn build_initialize_kiosk_ix(
    authority: &Pubkey,
    kiosk_id: u64,
    mint: &Pubkey,
    treasury: &Pubkey,
    fee_bps: u16,
    max_per_tx: u64,
) -> Instruction {
    let (kiosk, _) = kiosk_pda(kiosk_id);

    Instruction::new_with_bytes(
        changelink::id(),
        &changelink::instruction::InitializeKiosk {
            kiosk_id,
            fee_bps,
            max_per_tx,
        }
        .data(),
        changelink::accounts::InitializeKiosk {
            authority: *authority,
            kiosk,
            mint: *mint,
            treasury: *treasury,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn build_exchange_ix(
    authority: &Pubkey,
    kiosk: &Pubkey,
    treasury: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    serial: u64,
    gross: u64,
    currency: [u8; 3],
    rate_micro: u64,
) -> Instruction {
    let (receipt, _) = receipt_pda(kiosk, serial);

    Instruction::new_with_bytes(
        changelink::id(),
        &changelink::instruction::Exchange {
            serial,
            gross,
            currency,
            rate_micro,
        }
        .data(),
        changelink::accounts::Exchange {
            authority: *authority,
            kiosk: *kiosk,
            treasury: *treasury,
            mint: *mint,
            destination: *destination,
            receipt,
            token_program: token_program_id(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn build_set_limits_ix(
    authority: &Pubkey,
    kiosk: &Pubkey,
    fee_bps: u16,
    max_per_tx: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        changelink::id(),
        &changelink::instruction::SetLimits {
            fee_bps,
            max_per_tx,
        }
        .data(),
        changelink::accounts::SetLimits {
            authority: *authority,
            kiosk: *kiosk,
        }
        .to_account_metas(None),
    )
}

pub fn build_withdraw_treasury_ix(
    authority: &Pubkey,
    kiosk: &Pubkey,
    treasury: &Pubkey,
    destination: &Pubkey,
    amount: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        changelink::id(),
        &changelink::instruction::WithdrawTreasury { amount }.data(),
        changelink::accounts::WithdrawTreasury {
            authority: *authority,
            kiosk: *kiosk,
            treasury: *treasury,
            destination: *destination,
            token_program: token_program_id(),
        }
        .to_account_metas(None),
    )
}

// ------------------------------------------------- fixture

pub struct KioskFixture {
    pub operator: Keypair,
    pub mint: Keypair,
    pub treasury: Keypair,
    pub kiosk: Pubkey,
    pub kiosk_id: u64,
}

/// Stand up one machine with its own stablecoin mint and a funded treasury, exactly as the
/// operator would in the field.
pub fn setup_kiosk(
    svm: &mut LiteSVM,
    kiosk_id: u64,
    fee_bps: u16,
    max_per_tx: u64,
    treasury_balance: u64,
) -> KioskFixture {
    let operator = Keypair::new();
    fund(svm, &operator.pubkey(), 100 * ONE_SOL);

    let mint = create_mint(svm, &operator, 6);

    let (kiosk, _) = kiosk_pda(kiosk_id);
    // The treasury is owned by the kiosk PDA — that is the invariant the program enforces,
    // and the reason the machine can be the one to sign the payout.
    let treasury = create_token_account(svm, &operator, &mint.pubkey(), &kiosk);

    let ix = build_initialize_kiosk_ix(
        &operator.pubkey(),
        kiosk_id,
        &mint.pubkey(),
        &treasury.pubkey(),
        fee_bps,
        max_per_tx,
    );
    send(svm, &operator, &[ix], &[]).expect("initialize_kiosk should succeed");

    if treasury_balance > 0 {
        let mint_ix = spl_mint_to_ix(
            &mint.pubkey(),
            &treasury.pubkey(),
            &operator.pubkey(),
            treasury_balance,
        );
        send(svm, &operator, &[mint_ix], &[]).expect("funding the treasury should succeed");
    }

    KioskFixture {
        operator,
        mint,
        treasury,
        kiosk,
        kiosk_id,
    }
}

/// A traveller's own token account — where the stablecoin is supposed to land.
pub fn traveller_wallet(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey) -> Keypair {
    let traveller = Keypair::new();
    fund(svm, &traveller.pubkey(), ONE_SOL);
    create_token_account(svm, payer, mint, &traveller.pubkey())
}

pub fn kiosk_last_serial(svm: &LiteSVM, kiosk: &Pubkey) -> u64 {
    let acc = svm.get_account(kiosk).expect("kiosk should exist");
    let data = &acc.data;
    // authority(32) kiosk_id(8) mint(32) treasury(32) fee_bps(2) max_per_tx(8)
    // total_swaps(8) total_gross(8) last_serial(8) bump(1), after the 8-byte discriminator.
    let off = 8 + 32 + 8 + 32 + 32 + 2 + 8 + 8 + 8;
    u64::from_le_bytes(data[off..off + 8].try_into().unwrap())
}
