#!/usr/bin/env python3
"""ChangeLink — end-to-end smoke test against Solana devnet.

Does the whole product on the real chain, in this order:

  1. create a demo stablecoin mint (6 decimals)
  2. stand up a kiosk (serial 20261011, or the next free one) with a treasury owned by the
     kiosk PDA
  3. fund the treasury with 1,000 units
  4. settle one exchange: 100 units in, 98 units out to the traveller's own token account
  5. read the on-chain receipt and the machine's running totals back

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

    def send(self, ixs, payer: Keypair, extra_signers=(), label="") -> str:
        blockhash = self.client.get_latest_blockhash().value.blockhash
        tx = Transaction.new_signed_with_payer(
            ixs, payer.pubkey(), [payer, *extra_signers], blockhash
        )
        sig = self.client.send_transaction(
            tx, opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed")
        ).value
        self.confirm(sig, label)
        return str(sig)

    def confirm(self, sig, label="", timeout=120):
        deadline = time.time() + timeout
        while time.time() < deadline:
            status = self.client.get_signature_statuses([sig]).value[0]
            if status is not None:
                if status.err:
                    raise SystemExit(f"[FAIL] {label}: {status.err}")
                # at "confirmed" commitment a non-null status means it landed
                print(f"  ok  {label:26s} {sig}")
                return
            time.sleep(1.5)
        raise SystemExit(f"[FAIL] {label}: not confirmed within {timeout}s ({sig})")

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
        return self.client.get_minimum_balance_for_rent_exemption(size).value

    def account(self, addr):
        return self.client.get_account_info(addr).value

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
        f"balance  {chain.client.get_balance(authority.pubkey()).value / 1e9:.4f} SOL\n"
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

    # 4. a traveller, and one exchange
    traveller = Keypair()
    chain.create_data_account(
        authority, traveller, TOKEN_ACCOUNT_LEN, TOKEN_PROGRAM, "create traveller account"
    )
    chain.send(
        [ix_initialize_account3(traveller.pubkey(), mint.pubkey(), traveller.pubkey())],
        authority,
        label="initialize traveller acct",
    )
    ix = ix_exchange(authority.pubkey(), kiosk, treasury.pubkey(), mint.pubkey(), traveller.pubkey(), 1)
    sig_swap = chain.send([ix], authority, label="exchange (100 -> 98)")

    # 5. read it back off the chain
    fee = GROSS * FEE_BPS // 10_000
    net = GROSS - fee
    got = chain.token_amount(traveller.pubkey())
    left = chain.token_amount(treasury.pubkey())

    receipt, _ = Pubkey.find_program_address(
        [RECEIPT_SEED, bytes(kiosk), struct.pack("<Q", 1)], PROGRAM_ID
    )
    rdata = bytes(chain.wait_account(receipt).data)
    # re-read the machine *after* the swap: total_swaps and last_serial must have moved
    kiosk_data = bytes(chain.wait_account(kiosk).data)
    last_serial_off = 8 + 32 + 8 + 32 + 32 + 2 + 8 + 8 + 8
    total_swaps_off = 8 + 32 + 8 + 32 + 32 + 2 + 8

    print("\n--- on-chain result -------------------------------------------")
    print(f"  traveller wallet   {got / ONE:>10.2f}  (expected {net / ONE:.2f})")
    print(f"  kiosk treasury     {left / ONE:>10.2f}")
    print(f"  receipt PDA        {receipt}")
    print(f"    .destination     {Pubkey.from_bytes(rdata[48:80])}")
    print(f"    .gross/.net/.fee {u64_at(rdata, 80)} / {u64_at(rdata, 88)} / {u64_at(rdata, 96)}")
    print(f"    .currency        {rdata[104:107].decode()}  rate {u64_at(rdata, 107) / 1e6:.4f}")
    print(f"  kiosk.total_swaps  {u64_at(kiosk_data, total_swaps_off)}")
    print(f"  kiosk.last_serial  {u64_at(kiosk_data, last_serial_off)}")
    print("--------------------------------------------------------------")
    print(f"  program   https://explorer.solana.com/address/{PROGRAM_ID}?cluster=devnet")
    print(f"  init tx   https://explorer.solana.com/tx/{sig_init}?cluster=devnet")
    print(f"  swap tx   https://explorer.solana.com/tx/{sig_swap}?cluster=devnet")

    assert got == net, f"traveller got {got}, expected {net}"
    assert left == 1_000 * ONE - net, f"treasury holds {left}"
    assert Pubkey.from_bytes(rdata[48:80]) == traveller.pubkey(), "receipt destination mismatch"
    assert u64_at(rdata, 88) == net and u64_at(rdata, 96) == fee, "receipt amounts mismatch"
    assert u64_at(kiosk_data, last_serial_off) == 1, "kiosk.last_serial did not move"
    assert u64_at(kiosk_data, total_swaps_off) == 1, "kiosk.total_swaps did not move"
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
                "receipt": str(receipt),
                "init_tx": sig_init,
                "swap_tx": sig_swap,
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
