mod common;

use {common::*, solana_keypair::Keypair, solana_signer::Signer};

const FEE_BPS: u16 = 200;
const MAX_PER_TX: u64 = 500 * ONE_USDC;

/// Drive one settled swap so the machine has cash in its record to reconcile against.
fn settle_one(
    svm: &mut litesvm::LiteSVM,
    k: &KioskFixture,
    wallet: &Keypair,
    serial: u64,
    gross: u64,
) {
    let ix = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        serial,
        gross,
        *b"EUR",
        1_087_300,
    );
    send(svm, &k.operator, &[ix], &[]).expect("exchange should succeed");
}

#[test]
fn a_count_that_adds_up_leaves_the_machine_open() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    settle_one(&mut svm, &k, &wallet, 1, 100 * ONE_USDC);
    settle_one(&mut svm, &k, &wallet, 2, 60 * ONE_USDC);

    // 160 in the box, and the operator counts 160.
    let ix = build_attest_backing_ix(&k.operator.pubkey(), &k.kiosk, 160 * ONE_USDC, 0);
    send(&mut svm, &k.operator, &[ix], &[]).expect("attestation should succeed");

    assert!(
        !kiosk_frozen(&svm, &k.kiosk),
        "a full count must not freeze"
    );
}

#[test]
fn a_short_count_freezes_the_machine_and_stops_it_settling() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    settle_one(&mut svm, &k, &wallet, 1, 100 * ONE_USDC);

    // 100 went in; 60 is in the box. Well outside the 1% tolerance.
    let ix = build_attest_backing_ix(&k.operator.pubkey(), &k.kiosk, 60 * ONE_USDC, 0);
    send(&mut svm, &k.operator, &[ix], &[]).expect("the attestation itself is not an error");
    assert!(
        kiosk_frozen(&svm, &k.kiosk),
        "a short box must freeze the machine"
    );

    let before = token_amount(&svm, &wallet.pubkey());
    let ix = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        2,
        50 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    assert!(
        send(&mut svm, &k.operator, &[ix], &[]).is_err(),
        "a frozen machine must settle nothing"
    );
    assert_eq!(
        token_amount(&svm, &wallet.pubkey()),
        before,
        "nothing moved"
    );
}

#[test]
fn a_count_that_adds_up_reopens_a_frozen_machine() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    settle_one(&mut svm, &k, &wallet, 1, 100 * ONE_USDC);
    send(
        &mut svm,
        &k.operator,
        &[build_attest_backing_ix(
            &k.operator.pubkey(),
            &k.kiosk,
            10 * ONE_USDC,
            0,
        )],
        &[],
    )
    .unwrap();
    assert!(kiosk_frozen(&svm, &k.kiosk));

    // The missing notes turn up and are counted: the machine opens again.
    send(
        &mut svm,
        &k.operator,
        &[build_attest_backing_ix(
            &k.operator.pubkey(),
            &k.kiosk,
            100 * ONE_USDC,
            0,
        )],
        &[],
    )
    .expect("re-attesting is the un-freeze path");
    assert!(!kiosk_frozen(&svm, &k.kiosk));

    settle_one(&mut svm, &k, &wallet, 2, 50 * ONE_USDC);
}

#[test]
fn a_declared_removal_explains_a_smaller_count() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    settle_one(&mut svm, &k, &wallet, 1, 100 * ONE_USDC);

    // The operator took 60 out of the box and says so: 40 left is exactly right.
    let ix = build_attest_backing_ix(&k.operator.pubkey(), &k.kiosk, 40 * ONE_USDC, 60 * ONE_USDC);
    send(&mut svm, &k.operator, &[ix], &[]).expect("attestation should succeed");
    assert!(
        !kiosk_frozen(&svm, &k.kiosk),
        "a declared removal must not look like a short box"
    );

    // And taking out more than ever came in is still short.
    let ix = build_attest_backing_ix(&k.operator.pubkey(), &k.kiosk, 0, 0);
    send(&mut svm, &k.operator, &[ix], &[]).unwrap();
    assert!(
        kiosk_frozen(&svm, &k.kiosk),
        "an empty box with 40 expected is short"
    );
}

#[test]
fn only_the_operator_can_attest() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 0);

    let stranger = Keypair::new();
    fund(&mut svm, &stranger.pubkey(), 10 * ONE_SOL);
    let ix = build_attest_backing_ix(&stranger.pubkey(), &k.kiosk, 1_000 * ONE_USDC, 0);
    assert!(
        send(&mut svm, &stranger, &[ix], &[]).is_err(),
        "only the machine's own operator may attest"
    );
}
