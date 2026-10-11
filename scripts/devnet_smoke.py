#!/usr/bin/env python3
"""ChangeLink — end-to-end smoke test against Solana devnet.

Does the whole product on the real chain, in this order:

  1. create a demo stablecoin mint (6 decimals)
  2. stand up a kiosk (serial 20261011, or the next free one) with a treasury owned by the
     kiosk PDA
  3. fund the treasury with 1,000 units
  4. a traveller who has never held the stablecoin: the machine creates their token account
     for them (and pays the rent), then settles two exchanges of 100 units into it
  5. the same traveller signs 50 units back in and takes cash out — the other direction
  6. the operator counts the cash box, then counts it short: that freezes the machine, and
     the chain then refuses the next swap outright
  7. read the receipts, the machine's running totals and the freeze state back

Everything is hand-assembled: no Anchor client, no web3.js — just the byte layouts of the
System program, the SPL Token program and our own program. Same layouts the Rust tests use.

Usage
-----
    ~/.venvs/letterbox/bin/python scripts/devnet_smoke.py [--rpc URL] [--wallet PATH]

Notes
-----
* The default RPC is the local relay (see china-network-access / vps-proxy-selfhost):
  mainland networks cannot reach api.devnet.solana.com directly, and the Solana websocket
  never honours a proxy. Run it with `env -u all_proxy`.
* The wallet must hold devnet SOL (~0.1 SOL is plenty, plus rent for the new accounts).
"""

import argparse
import hashlib
import json
import struct
import sys
import time

from solders.instruction import AccountMeta, Instruction
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.transaction import Transaction

from solana.rpc.api import Client
from solana.rpc.types import TxOpts

PROGRAM_ID = Pubkey.from_string("4GA9eg49dVH8hpJmN7dkiodXNcAUuyi9wiXjeGkeUVCF")
TOKEN_PROGRAM = Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
ASSOCIATED_TOKEN_PROGRAM = Pubkey.from_string(
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
)
SYSTEM_PROGRAM = Pubkey.from_string("11111111111111111111111111111111")

KIOSK_SEED = b"kiosk"
RECEIPT_SEED = b"receipt"

DECIMALS = 6
ONE = 10**DECIMALS

KIOSK_ID = 20261011
FEE_BPS = 200          # 2%
MAX_PER_TX = 500 * ONE
GROSS = 100 * ONE
CURRENCY = b"EUR"
RATE_MICRO = 1_087_300

MINT_LEN = 82
TOKEN_ACCOUNT_LEN = 165
KIOSK_INIT_SPACE = 147  # 8-byte discriminator + 139 bytes of fields


# --------------------------------------------------------------- byte layouts

def anchor_disc(name: str) -> bytes:
    return hashlib.sha256(("global:" + name).encode()).digest()[:8]


def ix_create_account(frm, to, lamports, space, owner) -> Instruction:
    data = struct.pack("<IQQ", 0, lamports, space) + bytes(owner)
    return Instruction(
        SYSTEM_PROGRAM,
        data,
        [
            AccountMeta(frm, True, True),
            AccountMeta(to, True, True),
        ],
    )


def ix_initialize_mint2(mint, decimals, authority) -> Instruction:
    data = bytes([20, decimals]) + bytes(authority) + struct.pack("<I", 0)
    return Instruction(TOKEN_PROGRAM, data, [AccountMeta(mint, False, True)])


def ix_initialize_account3(account, mint, owner) -> Instruction:
    data = bytes([18]) + bytes(owner)
    return Instruction(
        TOKEN_PROGRAM,
        data,
        [AccountMeta(account, False, True), AccountMeta(mint, False, False)],
    )


def ix_mint_to(mint, to, authority, amount) -> Instruction:
    data = bytes([7]) + struct.pack("<Q", amount)
    return Instruction(
        TOKEN_PROGRAM,
        data,
        [
            AccountMeta(mint, False, True),
            AccountMeta(to, False, True),
            AccountMeta(authority, True, False),
        ],
    )


def ix_initialize_kiosk(authority, kiosk_id, mint, treasury) -> Instruction:
    kiosk, _ = Pubkey.find_program_address(
        [KIOSK_SEED, struct.pack("<Q", kiosk_id)], PROGRAM_ID
    )
    data = (
        anchor_disc("initialize_kiosk")
        + struct.pack("<Q", kiosk_id)
        + struct.pack("<H", FEE_BPS)
        + struct.pack("<Q", MAX_PER_TX)
    )
    return Instruction(
        PROGRAM_ID,
        data,
        [
            AccountMeta(authority, True, True),
            AccountMeta(kiosk, False, True),
            AccountMeta(mint, False, False),
            AccountMeta(treasury, False, False),
            AccountMeta(SYSTEM_PROGRAM, False, False),
        ],
    )


def ix_create_ata_idempotent(payer, wallet, mint) -> Instruction:
    """ATA program, `CreateIdempotent` (tag 1): gives `wallet` its token account for `mint`,
    or does nothing if it already has one. Only the payer signs — which is what lets the
    machine hand a traveller their first stablecoin from nothing but a wallet address."""
    ata = ata_for(wallet, mint)
    return Instruction(
        ASSOCIATED_TOKEN_PROGRAM,
        bytes([1]),
        [
            AccountMeta(payer, True, True),
            AccountMeta(ata, False, True),
            AccountMeta(wallet, False, False),
            AccountMeta(mint, False, False),
            AccountMeta(SYSTEM_PROGRAM, False, False),
            AccountMeta(TOKEN_PROGRAM, False, False),
        ],
    )


def ix_system_transfer(frm, to, lamports) -> Instruction:
    """System `Transfer` (instruction index 2)."""
    return Instruction(
        SYSTEM_PROGRAM,
        struct.pack("<IQ", 2, lamports),
        [AccountMeta(frm, True, True), AccountMeta(to, False, True)],
    )


def ix_redeem(traveller, kiosk, source, treasury, mint, serial, amount) -> Instruction:
    """The other direction: the traveller signs their own stablecoin back in, and the machine
    owes them notes. Traveller-signed on purpose — nothing moves without their key."""
    receipt, _ = Pubkey.find_program_address(
        [RECEIPT_SEED, bytes(kiosk), struct.pack("<Q", serial)], PROGRAM_ID
    )
    data = (
        anchor_disc("redeem")
        + struct.pack("<Q", serial)
        + struct.pack("<Q", amount)
        + CURRENCY
        + struct.pack("<Q", RATE_MICRO)
    )
    return Instruction(
        PROGRAM_ID,
        data,
        [
            AccountMeta(traveller, True, True),
            AccountMeta(kiosk, False, True),
            AccountMeta(source, False, True),
            AccountMeta(treasury, False, True),
            AccountMeta(mint, False, False),
            AccountMeta(receipt, False, True),
            AccountMeta(TOKEN_PROGRAM, False, False),
            AccountMeta(SYSTEM_PROGRAM, False, False),
        ],
    )


def ix_attest_backing(authority, kiosk, counted, removed) -> Instruction:
    """Operator-only: the physical count of the cash box, in the machine's bookkeeping unit."""
    data = (
        anchor_disc("attest_backing")
        + struct.pack("<Q", counted)
        + struct.pack("<Q", removed)
    )
    return Instruction(
        PROGRAM_ID,
        data,
        [AccountMeta(authority, True, False), AccountMeta(kiosk, False, True)],
    )


def ata_for(wallet, mint) -> Pubkey:
    return Pubkey.find_program_address(
        [bytes(wallet), bytes(TOKEN_PROGRAM), bytes(mint)], ASSOCIATED_TOKEN_PROGRAM
    )[0]


def ix_exchange(authority, kiosk, treasury, mint, destination, serial) -> Instruction:
    receipt, _ = Pubkey.find_program_address(
        [RECEIPT_SEED, bytes(kiosk), struct.pack("<Q", serial)], PROGRAM_ID
    )
    data = (
        anchor_disc("exchange")
        + struct.pack("<Q", serial)
        + struct.pack("<Q", GROSS)
        + CURRENCY
        + struct.pack("<Q", RATE_MICRO)
    )
    return Instruction(
        PROGRAM_ID,
        data,
        [
            AccountMeta(authority, True, True),
            AccountMeta(kiosk, False, True),
            AccountMeta(treasury, False, True),
            AccountMeta(mint, False, False),
            AccountMeta(destination, False, True),
            AccountMeta(receipt, False, True),
            AccountMeta(TOKEN_PROGRAM, False, False),
            AccountMeta(SYSTEM_PROGRAM, False, False),
        ],
    )


# --------------------------------------------------------------- RPC helpers

class Chain:
    def __init__(self, rpc: str):
        # commitment must be the *string* "confirmed": solana-py keys an internal lookup table
        # by string, and its default (finalized) reports a just-sent transaction as missing.
        self.client = Client(rpc, commitment="confirmed")

    @staticmethod
    def _try(fn, attempts=5, delay=2.0):
        """The public devnet RPC (and any relay in front of it) drops requests under load.
        Retry with backoff rather than failing a demo on a hiccup."""
        last = None
        for i in range(attempts):
            try:
                return fn()
            except Exception as e:
                last = e
                time.sleep(delay * (i + 1))
        raise SystemExit(f"RPC call failed {attempts} times in a row: {last}")

    def _submit(self, ixs, payer: Keypair, extra_signers=()):
        """One attempt: fresh blockhash, sign, submit. No retry — `send` wraps this."""
        blockhash = self.client.get_latest_blockhash().value.blockhash
        tx = Transaction.new_signed_with_payer(
            ixs, payer.pubkey(), [payer, *extra_signers], blockhash
        )
        return self.client.send_transaction(
            tx, opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed")
        ).value

    def send(self, ixs, payer: Keypair, extra_signers=(), label="") -> str:
        sig = self._try(lambda: self._submit(ixs, payer, extra_signers))
        self.confirm(sig, label)
        return str(sig)

    def confirm(self, sig, label="", timeout=120):
        deadline = time.time() + timeout
        while time.time() < deadline:
            status = self._try(
                lambda: self.client.get_signature_statuses([sig]), attempts=3, delay=1.5
            ).value[0]
            if status is not None:
                if status.err:
                    raise SystemExit(f"[FAIL] {label}: {status.err}")
                # at "confirmed" commitment a non-null status means it landed
                print(f"  ok  {label:26s} {sig}")
                return
            time.sleep(1.5)
        raise SystemExit(f"[FAIL] {label}: not confirmed within {timeout}s ({sig})")

    def refused(self, ixs, payer: Keypair, label=""):
        """Send a transaction the program is expected to refuse — once, no retry, because a
        refusal is deterministic. Returns the error text."""
        try:
            self._submit(ixs, payer)
        except Exception as e:
            reason = str(e).split("Error Message: ")[-1].split(".")[0][:52]
            print(f"  ok  {label:26s} refused: {reason}")
            return str(e)
        raise SystemExit(f"[FAIL] {label}: the chain accepted it, expected a refusal")

    def create_data_account(self, payer, kp, size, owner, label):
        """Create a plain owned account and wait until the cluster agrees it is `size` bytes,
        so the instruction that fills it in cannot race the creation."""
        self.send(
            [ix_create_account(payer.pubkey(), kp.pubkey(), self.rent(size), size, owner)],
            payer,
            [kp],
            label,
        )
        self.wait_account_size(kp.pubkey(), size)

    def rent(self, size: int) -> int:
        return self._try(
            lambda: self.client.get_minimum_balance_for_rent_exemption(size)
        ).value

    def account(self, addr):
        return self._try(lambda: self.client.get_account_info(addr)).value

    def wait_account(self, addr, timeout=60):
        """Devnet's public RPC is load balanced, so a just-confirmed account can still read
        as missing from the node that answers. Retry rather than believing the first None."""
        deadline = time.time() + timeout
        while True:
            info = self.account(addr)
            if info is not None or time.time() >= deadline:
                return info
            time.sleep(1.5)

    def wait_account_size(self, addr, size, timeout=60):
        """Same idea, but for the size as well. Without this, the follow-up
        `InitializeAccount3` can be processed by a node that still sees the fresh account as
        0 bytes and fail with a misleading `InvalidAccountData`."""
        deadline = time.time() + timeout
        while True:
            info = self.account(addr)
            if info is not None and len(bytes(info.data)) == size:
                return info
            if time.time() >= deadline:
                raise SystemExit(
                    f"account {addr} never reached {size} bytes (last: "
                    f"{'missing' if info is None else len(bytes(info.data))})"
                )
            time.sleep(1.0)

    def token_amount(self, addr) -> int:
        info = self.account(addr)
        if info is None:
            raise SystemExit(f"token account {addr} does not exist")
        return struct.unpack("<Q", bytes(info.data)[64:72])[0]


def u64_at(data: bytes, off: int) -> int:
    return struct.unpack("<Q", data[off : off + 8])[0]


# --------------------------------------------------------------- the run

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rpc", default="http://127.0.0.1:8899")
    ap.add_argument("--wallet", default="~/.config/solana/id.json")
    args = ap.parse_args()

    import pathlib

    authority = Keypair.from_json(
        pathlib.Path(args.wallet).expanduser().read_text()
    )
    chain = Chain(args.rpc)
    print(f"RPC      {args.rpc}")
    print(f"operator {authority.pubkey()}")
    print(
        f"balance  {chain._try(lambda: chain.client.get_balance(authority.pubkey())).value / 1e9:.4f} SOL\n"
    )

    # 1. the stablecoin this machine settles in
    mint = Keypair()
    chain.create_data_account(
        authority, mint, MINT_LEN, TOKEN_PROGRAM, "create mint account"
    )
    chain.send(
        [ix_initialize_mint2(mint.pubkey(), DECIMALS, authority.pubkey())],
        authority,
        label="initialize mint",
    )

    # 2. the machine — first free serial, so the script can be re-run safely
    for offset in range(50):
        kiosk_id = KIOSK_ID + offset
        kiosk, kiosk_bump = Pubkey.find_program_address(
            [KIOSK_SEED, struct.pack("<Q", kiosk_id)], PROGRAM_ID
        )
        if chain.account(kiosk) is not None:
            continue

        treasury = Keypair()
        chain.create_data_account(
            authority, treasury, TOKEN_ACCOUNT_LEN, TOKEN_PROGRAM, "create treasury account"
        )
        # Owned by the kiosk PDA — the invariant the program checks at init.
        chain.send(
            [ix_initialize_account3(treasury.pubkey(), mint.pubkey(), kiosk)],
            authority,
            label="initialize treasury",
        )
        try:
            ix = ix_initialize_kiosk(
                authority.pubkey(), kiosk_id, mint.pubkey(), treasury.pubkey()
            )
            sig_init = chain.send(
                [ix], authority, label=f"initialize_kiosk #{kiosk_id}"
            )
            break
        except SystemExit:
            print(f"  .. serial {kiosk_id} is already taken on-chain, next one")
    else:
        raise SystemExit("could not find a free kiosk serial")

    kiosk_info = chain.wait_account(kiosk)
    if kiosk_info is None:
        raise SystemExit("kiosk account was not created")
    print(f"  kiosk PDA {kiosk} (bump {kiosk_bump}), {len(bytes(kiosk_info.data))} bytes")

    # 3. float the machine
    chain.send(
        [ix_mint_to(mint.pubkey(), treasury.pubkey(), authority.pubkey(), 1_000 * ONE)],
        authority,
        label="fund treasury (1,000)",
    )

    # 4. a traveller who has never held this stablecoin. They hand over nothing but an
    #    address: the machine creates their token account (and pays the rent) as part of the
    #    settlement, exactly as the kiosk UI will once it talks to the program.
    traveller = Keypair()
    chain.send(
        [ix_system_transfer(authority.pubkey(), traveller.pubkey(), 10_000_000)],
        authority,
        label="fund traveller wallet",
    )
    ata = ata_for(traveller.pubkey(), mint.pubkey())

    fee = GROSS * FEE_BPS // 10_000
    net = GROSS - fee
    swaps = 2
    sig_swap = ""
    for serial in range(1, swaps + 1):
        # Idempotent: creates the account on the first swap, a no-op on the second.
        chain.send(
            [ix_create_ata_idempotent(authority.pubkey(), traveller.pubkey(), mint.pubkey())],
            authority,
            label=f"create traveller ATA #{serial}",
        )
        sig_swap = chain.send(
            [
                ix_exchange(
                    authority.pubkey(), kiosk, treasury.pubkey(), mint.pubkey(), ata, serial
                )
            ],
            authority,
            label=f"exchange #{serial} (100 -> 98)",
        )

    # 5. the other direction: the traveller signs part of it back in and takes cash out
    ata = ata_for(traveller.pubkey(), mint.pubkey())
    cash_back = 50 * ONE
    cash_fee = cash_back * FEE_BPS // 10_000
    cash_out = cash_back - cash_fee
    serial_redeem = swaps + 1
    sig_redeem = chain.send(
        [
            ix_redeem(
                traveller.pubkey(),
                kiosk,
                ata,
                treasury.pubkey(),
                mint.pubkey(),
                serial_redeem,
                cash_back,
            )
        ],
        traveller,
        label=f"redeem #{serial_redeem} (50 -> 49)",
    )

    # 6. the operator counts the box: everything taken in, less what the machine paid back out
    expected_backing = swaps * GROSS - cash_out
    chain.send(
        [ix_attest_backing(authority.pubkey(), kiosk, expected_backing, 0)],
        authority,
        label="attest a full count",
    )

    # 7. a box that comes up short freezes the machine — and the chain then refuses to settle
    chain.send(
        [ix_attest_backing(authority.pubkey(), kiosk, 0, 0)],
        authority,
        label="attest an empty box",
    )
    frozen = bytes(chain.wait_account(kiosk).data)[-2] != 0
    serial_refused = serial_redeem + 1
    chain.refused(
        [
            ix_exchange(
                authority.pubkey(),
                kiosk,
                treasury.pubkey(),
                mint.pubkey(),
                ata,
                serial_refused,
            )
        ],
        authority,
        label=f"swap #{serial_refused} while frozen",
    )

    # 8. read it all back off the chain
    got = chain.token_amount(ata)
    left = chain.token_amount(treasury.pubkey())
    owner = bytes(chain.wait_account(ata).data)[32:64]

    receipt, _ = Pubkey.find_program_address(
        [RECEIPT_SEED, bytes(kiosk), struct.pack("<Q", serial_redeem)], PROGRAM_ID
    )
    rdata = bytes(chain.wait_account(receipt).data)
    kiosk_data = bytes(chain.wait_account(kiosk).data)
    # Kiosk, after the 8-byte discriminator: authority(32) kiosk_id(8) accepted_mint(32)
    # treasury(32) fee_bps(2) max_per_tx(8) total_swaps(8) total_gross(8) last_serial(8)
    # total_removed(8) total_redeemed(8) counted_backing(8) backing_checked_at(8) frozen(1)
    total_swaps_off = 8 + 32 + 8 + 32 + 32 + 2 + 8
    last_serial_off = total_swaps_off + 8 + 8
    total_redeemed_off = last_serial_off + 8 + 8

    print("\n--- on-chain result -------------------------------------------")
    print(f"  traveller wallet   {traveller.pubkey()}")
    print(f"  their token acct   {ata}")
    print(f"    .owner (wallet)  {Pubkey.from_bytes(owner)}")
    print(f"    balance          {got / ONE:>7.2f}  (expected {(swaps * net - cash_back) / ONE:.2f})")
    print(f"  kiosk treasury     {left / ONE:>7.2f}")
    print(f"  cash-out receipt   {receipt}")
    print(f"    .destination     {Pubkey.from_bytes(rdata[48:80])}")
    print(f"    .gross/.net/.fee {u64_at(rdata, 80)} / {u64_at(rdata, 88)} / {u64_at(rdata, 96)}")
    print(f"    .kind            {rdata[-2]}  (1 = cash out)")
    print(f"  kiosk.total_swaps  {u64_at(kiosk_data, total_swaps_off)}")
    print(f"  kiosk.last_serial  {u64_at(kiosk_data, last_serial_off)}")
    print(f"  kiosk.redeemed     {u64_at(kiosk_data, total_redeemed_off) / ONE:.2f}")
    print(f"  kiosk.frozen       {frozen}")
    print("--------------------------------------------------------------")
    print(f"  program   https://explorer.solana.com/address/{PROGRAM_ID}?cluster=devnet")
    print(f"  kiosk tx  https://explorer.solana.com/tx/{sig_init}?cluster=devnet")
    print(f"  swap tx   https://explorer.solana.com/tx/{sig_swap}?cluster=devnet")
    print(f"  cash tx   https://explorer.solana.com/tx/{sig_redeem}?cluster=devnet")

    assert got == swaps * net - cash_back, f"traveller holds {got}"
    assert left == 1_000 * ONE - swaps * net + cash_back, f"treasury holds {left}"
    assert Pubkey.from_bytes(owner) == traveller.pubkey(), "the token account is not theirs"
    assert Pubkey.from_bytes(rdata[48:80]) == traveller.pubkey(), "receipt points elsewhere"
    assert u64_at(rdata, 88) == cash_out and u64_at(rdata, 96) == cash_fee, "cash-out amounts"
    assert rdata[-2] == 1, "the cash-out receipt must be marked as one"
    assert u64_at(kiosk_data, total_redeemed_off) == cash_out, "total_redeemed did not move"
    assert u64_at(kiosk_data, last_serial_off) == serial_redeem, "a refused swap bumped it"
    assert frozen, "a short box must leave the machine frozen"
    print("\nAll devnet assertions passed.")

    # Handy for the README / form fields.
    print(
        json.dumps(
            {
                "program_id": str(PROGRAM_ID),
                "kiosk_pda": str(kiosk),
                "mint": str(mint.pubkey()),
                "treasury": str(treasury.pubkey()),
                "traveller_wallet": str(traveller.pubkey()),
                "traveller_token_account": str(ata),
                "cash_out_receipt": str(receipt),
                "kiosk_tx": sig_init,
                "swap_tx": sig_swap,
                "cash_out_tx": sig_redeem,
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
