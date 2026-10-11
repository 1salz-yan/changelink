mod common;

use {common::*, solana_keypair::Keypair, solana_signer::Signer};

const FEE_BPS: u16 = 200; // 2%, the product's demo fee
const MAX_PER_TX: u64 = 500 * ONE_USDC;

#[test]
fn an_exchange_pays_the_traveller_own_wallet_and_keeps_a_receipt() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    let gross = 100 * ONE_USDC;
    let ix = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        gross,
        *b"EUR",
        1_087_300,
    );
    send(&mut svm, &k.operator, &[ix], &[]).expect("exchange should succeed");

    let fee = gross * FEE_BPS as u64 / 10_000;
    let net = gross - fee;

    // The money is in the traveller's own account. The machine holds no key to it.
    assert_eq!(token_amount(&svm, &wallet.pubkey()), net);
    assert_eq!(
        token_amount(&svm, &k.treasury.pubkey()),
        1_000 * ONE_USDC - net,
        "the payout left the kiosk treasury"
    );

    let (receipt, _) = receipt_pda(&k.kiosk, 1);
    let acc = svm.get_account(&receipt).expect("receipt should exist");
    assert_eq!(acc.owner, changelink::id());
    let d = &acc.data;
    assert_eq!(&d[8..40], k.kiosk.as_ref(), "receipt.kiosk");
    assert_eq!(
        u64::from_le_bytes(d[40..48].try_into().unwrap()),
        1,
        "serial"
    );
    assert_eq!(
        &d[48..80],
        wallet.pubkey().as_ref(),
        "receipt.destination is the wallet the money went to"
    );
    assert_eq!(u64::from_le_bytes(d[80..88].try_into().unwrap()), gross);
    assert_eq!(u64::from_le_bytes(d[88..96].try_into().unwrap()), net);
    assert_eq!(u64::from_le_bytes(d[96..104].try_into().unwrap()), fee);
    assert_eq!(&d[104..107], b"EUR");
    assert_eq!(
        u64::from_le_bytes(d[107..115].try_into().unwrap()),
        1_087_300
    );

    assert_eq!(kiosk_last_serial(&svm, &k.kiosk), 1, "serial counter moved");
}

#[test]
fn an_amount_over_the_per_transaction_limit_is_refused() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    let ix = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        MAX_PER_TX + 1,
        *b"EUR",
        1_087_300,
    );
    assert!(
        send(&mut svm, &k.operator, &[ix], &[]).is_err(),
        "one base unit over the limit must be refused"
    );

    assert_eq!(token_amount(&svm, &wallet.pubkey()), 0, "nothing moved");
    assert_eq!(
        token_amount(&svm, &k.treasury.pubkey()),
        1_000 * ONE_USDC,
        "the treasury is untouched"
    );
    assert!(svm.get_account(&receipt_pda(&k.kiosk, 1).0).is_none());
}

#[test]
fn exactly_the_limit_is_allowed() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    let ix = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        MAX_PER_TX,
        *b"EUR",
        1_087_300,
    );
    send(&mut svm, &k.operator, &[ix], &[]).expect("the limit is inclusive");

    let fee = MAX_PER_TX * FEE_BPS as u64 / 10_000;
    assert_eq!(token_amount(&svm, &wallet.pubkey()), MAX_PER_TX - fee);
}

#[test]
fn settling_the_same_serial_twice_is_refused() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    let ix = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        100 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    send(&mut svm, &k.operator, &[ix], &[]).expect("first settlement");

    let balance_after_first = token_amount(&svm, &wallet.pubkey());

    // The same note intake, replayed. The receipt PDA already exists, so `init` refuses it.
    let replay = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        100 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    assert!(
        send(&mut svm, &k.operator, &[replay], &[]).is_err(),
        "a serial must only ever settle once"
    );
    assert_eq!(
        token_amount(&svm, &wallet.pubkey()),
        balance_after_first,
        "the replayed settlement must not pay out a second time"
    );
}

#[test]
fn a_real_second_swap_with_a_new_serial_still_works() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    for serial in 1..=3u64 {
        let ix = build_exchange_ix(
            &k.operator.pubkey(),
            &k.kiosk,
            &k.treasury.pubkey(),
            &k.mint.pubkey(),
            &wallet.pubkey(),
            serial,
            50 * ONE_USDC,
            *b"EUR",
            1_087_300,
        );
        send(&mut svm, &k.operator, &[ix], &[]).expect("each new serial settles");
    }

    let net_per_swap = 50 * ONE_USDC - 50 * ONE_USDC * FEE_BPS as u64 / 10_000;
    assert_eq!(token_amount(&svm, &wallet.pubkey()), 3 * net_per_swap);
    assert_eq!(kiosk_last_serial(&svm, &k.kiosk), 3);
}

#[test]
fn only_the_machines_operator_key_can_settle() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, MAX_PER_TX, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    let stranger = Keypair::new();
    fund(&mut svm, &stranger.pubkey(), 10 * ONE_SOL);

    let ix = build_exchange_ix(
        &stranger.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        100 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    assert!(
        send(&mut svm, &stranger, &[ix], &[]).is_err(),
        "the kiosk authority is the only key allowed to move money out"
    );
    assert_eq!(token_amount(&svm, &wallet.pubkey()), 0);
}

#[test]
fn the_cap_is_live_config_not_a_constant() {
    let mut svm = setup_svm();
    let k = setup_kiosk(&mut svm, 7, FEE_BPS, 100 * ONE_USDC, 1_000 * ONE_USDC);
    let wallet = traveller_wallet(&mut svm, &k.operator, &k.mint.pubkey());

    // 150 USDC is over today's cap.
    let over = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        150 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    assert!(send(&mut svm, &k.operator, &[over], &[]).is_err());

    // The operator raises the ceiling; the same amount now settles.
    let raise = build_set_limits_ix(&k.operator.pubkey(), &k.kiosk, FEE_BPS, 200 * ONE_USDC);
    send(&mut svm, &k.operator, &[raise], &[]).expect("operator may raise the ceiling");

    let again = build_exchange_ix(
        &k.operator.pubkey(),
        &k.kiosk,
        &k.treasury.pubkey(),
        &k.mint.pubkey(),
        &wallet.pubkey(),
        1,
        150 * ONE_USDC,
        *b"EUR",
        1_087_300,
    );
    send(&mut svm, &k.operator, &[again], &[]).expect("within the new ceiling");

    let fee = 150 * ONE_USDC * FEE_BPS as u64 / 10_000;
    assert_eq!(token_amount(&svm, &wallet.pubkey()), 150 * ONE_USDC - fee);
}
