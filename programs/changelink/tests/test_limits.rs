mod common;

use {common::*, solana_keypair::Keypair, solana_signer::Signer};

const FEE_BPS: u16 = 200;
const MAX_PER_TX: u64 = 500 * ONE_USDC;

#[test]
fn the_operator_can_sweep_collected_fees_out_of_the_treasury() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);

    let operator_account = create_token_account(
        &mut svm,
        &k.operator,
        &k.mint.pubkey(),
        &k.operator.pubkey(),
    );

    let ix = build_withdraw_treasury_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &operator_account.pubkey(),
        250 * ONE_USDC,
    );
    send(&mut svm, &k.operator, &[ix], &[]).expect("the operator may sweep");

    assert_eq!(
        token_amount(&svm, &operator_account.pubkey()),
        250 * ONE_USDC
    );
    assert_eq!(token_amount(&svm, &k.treasury.pubkey()), 750 * ONE_USDC);
}

#[test]
fn a_stranger_cannot_sweep_the_treasury() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);

    let stranger = Keypair::new();
    fund(&mut svm, &stranger.pubkey(), 10 * ONE_SOL);
    let stranger_account =
        create_token_account(&mut svm, &stranger, &k.mint.pubkey(), &stranger.pubkey());

    let ix = build_withdraw_treasury_ix(
        &stranger.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &stranger_account.pubkey(),
        250 * ONE_USDC,
    );
    assert!(
        send(&mut svm, &stranger, &[ix], &[]).is_err(),
        "sweeping is authority-gated"
    );
    assert_eq!(token_amount(&svm, &k.treasury.pubkey()), 1_000 * ONE_USDC);
}

#[test]
fn only_the_operator_can_change_the_limits() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 0);

    let stranger = Keypair::new();
    fund(&mut svm, &stranger.pubkey(), 10 * ONE_SOL);

    let ix = build_set_limits_ix(&stranger.pubkey(), &k.kiosk, 2_000, 10_000 * ONE_USDC);
    assert!(
        send(&mut svm, &stranger, &[ix], &[]).is_err(),
        "a stranger must not be able to raise the fee or the ceiling"
    );
}

#[test]
fn even_the_operator_cannot_exceed_the_fee_ceiling() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 0);

    let ix = build_set_limits_ix(&k.operator.pubkey(), &k.kiosk, 1_001, MAX_PER_TX);
    assert!(
        send(&mut svm, &k.operator, &[ix], &[]).is_err(),
        "MAX_FEE_BPS is enforced on every change, not only at initialisation"
    );
}

#[test]
fn a_machine_serial_can_be_re_initialised_by_a_different_operator_key() {
    // Two machines, two operators, same program: the PDAs are independent.
    let mut svm = setup_svm();
    let a = setup_kiosk(&mut svm, 1, FEE_BPS, MAX_PER_TX, 0);
    let b = setup_kiosk(&mut svm, 2, FEE_BPS, MAX_PER_TX, 0);

    assert_ne!(a.kiosk, b.kiosk, "each serial is its own config account");
    assert_ne!(a.treasury.pubkey(), b.treasury.pubkey());
    assert!(svm.get_account(&a.kiosk).is_some());
    assert!(svm.get_account(&b.kiosk).is_some());
}
