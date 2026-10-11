mod common;

use {
    changelink::state::{KIND_EXCHANGE, KIND_REDEEM},
    common::*,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

const FEE_BPS: u16 = 200;
const MAX_PER_TX: u64 = 500 * ONE_USDC;

/// Give the traveller stablecoin to cash back in, the way a previous swap would have.
fn give_stablecoin(
    svm: &mut litesvm::LiteSVM,
    k: &KioskFixture,
    to: &anchor_lang::prelude::Pubkey,
    amount: u64,
) {
    let ix = spl_mint_to_ix(&k.mint.pubkey(), to, &k.operator.pubkey(), amount);
    send(svm, &k.operator, &[ix], &[]).expect("minting to the traveller should succeed");
}

#[test]
fn a_traveller_can_hand_stablecoin_back_and_take_cash_out() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 100 * ONE_USDC);
    let t = new_traveller(&mut svm, &k.operator, &k.mint.pubkey());

    give_stablecoin(&mut svm, &k, &t.token_account.pubkey(), 100 * ONE_USDC);

    let ix = build_redeem_ix(
        &t.wallet.pubkey(),
        &k.kiosk,
        &t.token_account.pubkey(),
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        1,
        100 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    send(&mut svm, &t.wallet, &[ix], &[]).expect("redeem should succeed");

    let fee = 100 * ONE_USDC * FEE_BPS as u64 / 10_000;
    let payout = 100 * ONE_USDC - fee;

    assert_eq!(
        token_amount(&svm, &t.token_account.pubkey()),
        0,
        "the traveller's stablecoin went back to the machine"
    );
    assert_eq!(
        token_amount(&svm, &k.treasury.pubkey()),
        100 * ONE_USDC + 100 * ONE_USDC,
        "the machine's treasury holds what came back"
    );

    // The receipt is the machine's proof it owes notes for this swap.
    let (receipt, _) = receipt_pda(&k.kiosk, 1);
    let d = svm
        .get_account(&receipt)
        .expect("receipt should exist")
        .data;
    assert_eq!(
        &d[48..80],
        t.wallet.pubkey().as_ref(),
        "receipt points at the traveller"
    );
    assert_eq!(
        u64::from_le_bytes(d[80..88].try_into().unwrap()),
        100 * ONE_USDC
    );
    assert_eq!(u64::from_le_bytes(d[88..96].try_into().unwrap()), payout);
    assert_eq!(u64::from_le_bytes(d[96..104].try_into().unwrap()), fee);
    assert_eq!(
        d[d.len() - 2],
        KIND_REDEEM,
        "a cash-out receipt must be marked as one"
    );
    assert_ne!(KIND_REDEEM, KIND_EXCHANGE);
}

#[test]
fn cash_out_is_capped_well_below_cash_in() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 100 * ONE_USDC);
    let t = new_traveller(&mut svm, &k.operator, &k.mint.pubkey());

    // The cash-in ceiling is 500; cash out stops at a quarter of it, i.e. 125.
    give_stablecoin(&mut svm, &k, &t.token_account.pubkey(), 200 * ONE_USDC);

    let ix = build_redeem_ix(
        &t.wallet.pubkey(),
        &k.kiosk,
        &t.token_account.pubkey(),
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        1,
        126 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    assert!(
        send(&mut svm, &t.wallet, &[ix], &[]).is_err(),
        "one unit over the cash-out cap must be refused"
    );
    assert_eq!(
        token_amount(&svm, &t.token_account.pubkey()),
        200 * ONE_USDC,
        "nothing moved"
    );

    // At exactly the cap it goes through.
    let ix = build_redeem_ix(
        &t.wallet.pubkey(),
        &k.kiosk,
        &t.token_account.pubkey(),
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        1,
        125 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    send(&mut svm, &t.wallet, &[ix], &[]).expect("the cap is inclusive");
}

#[test]
fn a_serial_cannot_be_redeemed_twice() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 100 * ONE_USDC);
    let t = new_traveller(&mut svm, &k.operator, &k.mint.pubkey());

    give_stablecoin(&mut svm, &k, &t.token_account.pubkey(), 100 * ONE_USDC);
    let build = |serial: u64| {
        build_redeem_ix(
            &t.wallet.pubkey(),
            &k.kiosk,
            &t.token_account.pubkey(),
            &k.treasury.pubkey(),
            &k.mint.pubkey(),
            serial,
            50 * ONE_USDC,
            *b"EUR",
            1_087_300,
        )
    };

    send(&mut svm, &t.wallet, &[build(1)], &[]).expect("first cash out");
    let after = token_amount(&svm, &t.token_account.pubkey());

    assert!(
        send(&mut svm, &t.wallet, &[build(1)], &[]).is_err(),
        "the same serial must not be redeemed twice — the machine would pay twice"
    );
    assert_eq!(token_amount(&svm, &t.token_account.pubkey()), after);
}

#[test]
fn a_stranger_cannot_move_someone_elses_stablecoin_back() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 100 * ONE_USDC);
    let t = new_traveller(&mut svm, &k.operator, &k.mint.pubkey());
    give_stablecoin(&mut svm, &k, &t.token_account.pubkey(), 100 * ONE_USDC);

    let stranger = Keypair::new();
    fund(&mut svm, &stranger.pubkey(), 10 * ONE_SOL);

    // The stranger signs, but the token account belongs to the traveller: the Token program
    // refuses, which is the point of making this instruction traveller-signed.
    let ix = build_redeem_ix(
        &stranger.pubkey(),
        &k.kiosk,
        &t.token_account.pubkey(),
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        1,
        50 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    assert!(send(&mut svm, &stranger, &[ix], &[]).is_err());
    assert_eq!(
        token_amount(&svm, &t.token_account.pubkey()),
        100 * ONE_USDC
    );
}

#[test]
fn a_frozen_machine_takes_no_cash_out_either() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 100 * ONE_USDC);
    let t = new_traveller(&mut svm, &k.operator, &k.mint.pubkey());
    give_stablecoin(&mut svm, &k, &t.token_account.pubkey(), 100 * ONE_USDC);

    // One swap in, so the chain expects 100 in the box...
    let seller = new_traveller(&mut svm, &k.operator, &k.mint.pubkey());
    let swap = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &seller.token_account.pubkey(),
        1,
        100 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    send(&mut svm, &k.operator, &[swap], &[]).expect("the swap settles");

    // ...and the operator counts an empty box. That freezes the machine.
    let attest = build_attest_backing_ix(&k.operator.pubkey(), &k.kiosk, 0, 0);
    send(&mut svm, &k.operator, &[attest], &[]).unwrap();
    assert!(kiosk_frozen(&svm, &k.kiosk));

    // The serial counter is shared by both directions, so this is swap number 2.
    let ix = build_redeem_ix(
        &t.wallet.pubkey(),
        &k.kiosk,
        &t.token_account.pubkey(),
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        2,
        50 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    assert!(
        send(&mut svm, &t.wallet, &[ix], &[]).is_err(),
        "a frozen machine dispenses nothing, in either direction"
    );
}
