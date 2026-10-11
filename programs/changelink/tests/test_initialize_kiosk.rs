mod common;

use {changelink::state::Kiosk, common::*, solana_keypair::Keypair, solana_signer::Signer};

use anchor_lang::Space;

#[test]
fn initialize_kiosk_records_the_machine_and_its_treasury() {
    let mut svm = setup_svm();
    let operator = Keypair::new();
    fund(&mut svm, &operator.pubkey(), 10 * ONE_SOL);

    let mint = create_mint(&mut svm, &operator, 6);
    let (kiosk, expected_bump) = kiosk_pda(7);
    // The operator's payout account must be owned by the kiosk PDA.
    let treasury = create_token_account(&mut svm, &operator, &mint.pubkey(), &kiosk);

    let ix = build_initialize_kiosk_ix(
        &operator.pubkey(),
        7,
        &mint.pubkey(),
        &treasury.pubkey(),
        200,
        500 * ONE_USDC,
    );
    send(&mut svm, &operator, &[ix], &[]).expect("initialize should succeed");

    let acc = svm.get_account(&kiosk).expect("kiosk account should exist");
    assert_eq!(
        acc.owner,
        changelink::id(),
        "kiosk must be owned by the program"
    );
    assert!(acc.lamports > 0, "kiosk must be rent exempt");
    assert_eq!(
        acc.data.len(),
        8 + Kiosk::INIT_SPACE,
        "kiosk account must be exactly the size the struct declares"
    );

    let data = &acc.data;
    // Borsh, in field order, after the 8-byte Anchor discriminator.
    assert_eq!(
        &data[8..40],
        operator.pubkey().as_ref(),
        "authority is the first field"
    );
    assert_eq!(
        u64::from_le_bytes(data[40..48].try_into().unwrap()),
        7,
        "kiosk_id is the machine serial (and the PDA seed)"
    );
    assert_eq!(&data[48..80], mint.pubkey().as_ref(), "accepted_mint");
    assert_eq!(&data[80..112], treasury.pubkey().as_ref(), "treasury");
    assert_eq!(
        u16::from_le_bytes(data[112..114].try_into().unwrap()),
        200,
        "fee_bps"
    );
    assert_eq!(
        u64::from_le_bytes(data[114..122].try_into().unwrap()),
        500 * ONE_USDC,
        "max_per_tx"
    );
    assert_eq!(
        data[data.len() - 1],
        expected_bump,
        "bump is the last field"
    );
}

#[test]
fn initializing_the_same_machine_serial_twice_fails() {
    let mut svm = setup_svm();
    let operator = Keypair::new();
    fund(&mut svm, &operator.pubkey(), 10 * ONE_SOL);

    let mint = create_mint(&mut svm, &operator, 6);
    let (kiosk, _) = kiosk_pda(7);
    let treasury = create_token_account(&mut svm, &operator, &mint.pubkey(), &kiosk);

    let ix = build_initialize_kiosk_ix(
        &operator.pubkey(),
        7,
        &mint.pubkey(),
        &treasury.pubkey(),
        200,
        500 * ONE_USDC,
    );
    send(&mut svm, &operator, &[ix.clone()], &[]).expect("first initialize should succeed");

    let result = send(&mut svm, &operator, &[ix], &[]);
    assert!(
        result.is_err(),
        "the same machine serial must not be claimable twice"
    );
}

#[test]
fn a_fee_above_the_programs_ceiling_is_rejected() {
    let mut svm = setup_svm();
    let operator = Keypair::new();
    fund(&mut svm, &operator.pubkey(), 10 * ONE_SOL);

    let mint = create_mint(&mut svm, &operator, 6);
    let (kiosk, _) = kiosk_pda(7);
    let treasury = create_token_account(&mut svm, &operator, &mint.pubkey(), &kiosk);

    // 1001 bps = 10.01% — one basis point over the hard ceiling in constants.rs.
    let ix = build_initialize_kiosk_ix(
        &operator.pubkey(),
        7,
        &mint.pubkey(),
        &treasury.pubkey(),
        1001,
        500 * ONE_USDC,
    );
    assert!(
        send(&mut svm, &operator, &[ix], &[]).is_err(),
        "a fee over MAX_FEE_BPS must be refused"
    );
    assert!(
        svm.get_account(&kiosk).is_none(),
        "a rejected initialize must not leave a kiosk account behind"
    );
}

#[test]
fn a_treasury_the_program_cannot_sign_for_is_rejected() {
    let mut svm = setup_svm();
    let operator = Keypair::new();
    fund(&mut svm, &operator.pubkey(), 10 * ONE_SOL);

    let mint = create_mint(&mut svm, &operator, 6);
    // Owned by the operator, not by the kiosk PDA: the program could never move money out of
    // it, so it must refuse to record it.
    let treasury = create_token_account(&mut svm, &operator, &mint.pubkey(), &operator.pubkey());

    let ix = build_initialize_kiosk_ix(
        &operator.pubkey(),
        7,
        &mint.pubkey(),
        &treasury.pubkey(),
        200,
        500 * ONE_USDC,
    );
    assert!(
        send(&mut svm, &operator, &[ix], &[]).is_err(),
        "treasury must be owned by the kiosk PDA"
    );
}

#[test]
fn a_treasury_in_the_wrong_currency_is_rejected() {
    let mut svm = setup_svm();
    let operator = Keypair::new();
    fund(&mut svm, &operator.pubkey(), 10 * ONE_SOL);

    let mint = create_mint(&mut svm, &operator, 6);
    let other_mint = create_mint(&mut svm, &operator, 6);
    let (kiosk, _) = kiosk_pda(7);
    let treasury = create_token_account(&mut svm, &operator, &other_mint.pubkey(), &kiosk);

    let ix = build_initialize_kiosk_ix(
        &operator.pubkey(),
        7,
        &mint.pubkey(),
        &treasury.pubkey(),
        200,
        500 * ONE_USDC,
    );
    assert!(
        send(&mut svm, &operator, &[ix], &[]).is_err(),
        "the treasury must hold the stablecoin the kiosk settles in"
    );
}
