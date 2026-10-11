# ChangeLink — leftover foreign cash, straight into your own wallet, on Solana

A self-service kiosk for the foreign coins and banknotes a traveller can't spend
any more. Drop in the notes, scan your own wallet's receive code, and the same
value arrives as **USDC in your wallet** — no sign-up, no account, no phone number.
Hand dollars back and the machine gives you notes again, so the money is not a
one-way door.

Two layers. **`programs/changelink/`** is the settlement layer — an Anchor/Rust program,
deployed on devnet ([evidence below](#deployed-on-devnet)). **`atm.html`** is the kiosk UI:
one self-contained file, no build step, no framework, renders offline.

Built for Colosseum **Crypto World's Fair** (Oct 2026).

---

## The problem

Every traveller ends up with a handful of foreign banknotes and coins. Bureaux de
change mostly won't buy coins at all, the ones that do charge 3–10%, and whatever's
left is carried home to rot in a drawer. The money is already spent — it's just
stranded.

**Prior art, stated honestly:** [TravelersBox](https://www.travelersbox.com/) has run
coin/kiosk exchange machines at airports since 2012 and is still operating. We're not
claiming an empty field. The contrast is the settlement layer and the exit:
TravelersBox pays out gift cards and donations; we settle the value **on Solana
directly into the traveller's own wallet**.

**A closer one, also stated honestly:**
[Takarabako](https://github.com/AqilJaafree/takarabako) won an ETHGlobal Tokyo 2026 track
with the same idea on Ethereum — a kiosk that turns leftover notes into dollar savings.
They are ahead of us on several axes and it is worth naming them precisely, because a
judge who knows that project will:

- a **real bill acceptor** on a Raspberry Pi (ICT protocol), not a screen;
- **cash out as well as cash in**, plus a public proof-of-reserve dashboard for operators;
- **a device-generated key signs every accepted note** (`NoteAccepted: kiosk, amount,
  currency, session, nonce, time`), with the public key published in ENS, so a replayed
  signature cannot create money.

That last one is the one to be careful about: our receipts are signed by the machine's
**operator** key, not by the device, so on that axis they are ahead and we are not going
to pretend otherwise. If we build it, the honest claim is only a **mechanism** difference:
their public key is resolved through ENS, ours would live in the machine's own on-chain
account (the `Kiosk` PDA) and be checked by the program that pays out — no resolver in
the path. What we have today that they do not is the other axis: the value lands in the
traveller's **own** wallet. They create the wallet for you and hold the balance; there is
no account here to hold.

---

## Try it

```
open atm.html          # macOS — or just double-click it
```

No server, no install. three.js **is inlined into the file** (r128, MIT), so the 3D
kiosk renders offline.

The flow is five steps, and nothing is sent until the traveller confirms:

1. **Currency** — pick the currency you're holding; each carries its own cap.
2. **Notes** — drag a banknote from the rack into the intake slot. The machine
   verifies each note; anything that fails is physically ejected.
3. **Wallet** — type your wallet's receive address on the on-screen keyboard
   (base58 only, so there is no `0/O/I/l` to mistype), or scan the QR code.
4. **Confirm** — the machine shows the final offer (you insert X → you receive Y
   USDC, at the locked rate, fee, destination) with the quote countdown.
5. **Done** — amount, the rate it settled at, a receipt reference, and the
   transaction hash.

The interface is English by default with a 中文 toggle in the corner — an airport
machine has to be multilingual, and the submission is read in English.

---

## The settlement layer, on chain

The money movement is not in the browser. `programs/changelink/` is an Anchor/Rust program
that does it, and it is deployed on devnet. Four instructions:

| Instruction | Signer | What it does |
| --- | --- | --- |
| `initialize_kiosk` | operator | One config account per physical machine, seeded by the machine serial: the stablecoin it settles in, its treasury, its fee, its ceiling. |
| `exchange` | the machine | Cash in: an SPL Token CPI pays the traveller's own token account, and a receipt account is created. |
| `redeem` | **the traveller** | Cash out: the traveller signs their own stablecoin back into the machine's treasury and the machine owes them notes. |
| `set_limits` | operator | Changes the fee and the ceiling — bounded by the program's own maximum. |
| `withdraw_treasury` | operator | Sweeps collected value out of the machine's treasury. |
| `attest_backing` | operator | Records a physical count of the cash box. A count that comes up short freezes the machine; a count that adds up unfreezes it. |

**Accounts.** `Kiosk` — a PDA seeded by the word `kiosk` and the machine serial. `Receipt` —
a PDA seeded by the word `receipt`, the kiosk, and the swap serial.

**The invariants are in the code, not in the copy:**

1. **The per-transaction ceiling is checked on chain, before any token moves.** In the UI
   the limit is a per-currency table (JPY 10,000 / USD 20 / EUR 50 …); in the program it is
   a single `max_per_tx` per machine in stablecoin base units. Raising it is an on-chain
   instruction signed by the operator — not a client-side decision, and not something the
   machine can bend per customer.
2. **`MAX_FEE_BPS = 1000` (10%) is a hard ceiling**, applied when a machine is stood up and
   again on every later `set_limits`. No configuration can exceed it.
3. **The treasury must be owned by the kiosk account.** A machine cannot be pointed at an
   account the program cannot sign for, and the operator cannot drain it with a wallet
   signature — only through `withdraw_treasury`.
4. **A receipt account per (kiosk, serial) is the replay guard.** Creating it fails if it
   already exists, so the same note intake cannot be settled twice; a monotonic serial
   counter is the second, cheaper guard.
5. **The payout target is an argument, not a constant** — in production, the wallet the
   traveller just scanned. There is no hard-coded destination to get quietly wrong.
6. **Cash out is capped at a quarter of cash in.** A machine that runs out of notes
   mid-swap has taken the traveller's stablecoin for nothing, so the direction that can
   strand someone is the one held tightest (`max_per_tx / 4`). Cash out is also the only
   instruction the **traveller** signs: their own token account is the source.
7. **A machine whose cash box does not add up stops.** `attest_backing` takes the operator's
   physical count; if it is short of what the chain says was taken in (less declared
   removals and cash already paid out, within 1%) the machine sets `frozen`, and both
   `exchange` and `redeem` then refuse. Re-attesting a count that adds up is the un-freeze
   path. Being frozen is the outcome, not an error — stopping is the whole point.

We are **not** claiming "supply equals reserve" the way a custodial kiosk can: with the
value gone to the traveller's own wallet we hold no claim to back. What we replicate is the
mechanism that makes a cash machine trustworthy — an operator-signed physical count, and an
automatic freeze when the count disagrees with the ledger.

```bash
cargo build-sbf                     # the deployable binary (see the note below)
cargo test                          # 27 tests, against a real SPL Token program
```

```
running 5 tests ... 5 passed        (test_initialize_kiosk.rs)
running 7 tests ... 7 passed        (test_exchange.rs)
running 5 tests ... 5 passed        (test_redeem.rs)
running 5 tests ... 5 passed        (test_backing.rs)
running 5 tests ... 5 passed        (test_limits.rs)
```

The tests are not mocks. They load the compiled `.so` into LiteSVM, which provides the real
SPL Token program, so `token::transfer` genuinely moves tokens between token accounts.

> **Build note.** `anchor build` fetches the newest platform-tools, whose SBPF version
> devnet does not enable yet (`ELF error: Detected sbpf_version required by the executable
> which are not enabled`). Build the deployable binary with `cargo build-sbf` (default
> `--arch v0`) before `solana program deploy`; `anchor build` is fine for the local tests.

## Deployed on devnet

Program: [`4GA9eg49dVH8hpJmN7dkiodXNcAUuyi9wiXjeGkeUVCF`](https://explorer.solana.com/address/4GA9eg49dVH8hpJmN7dkiodXNcAUuyi9wiXjeGkeUVCF?cluster=devnet)

`scripts/devnet_smoke.py` runs the whole product against the real chain: it creates a
stablecoin mint, stands up a machine, floats its treasury with 1,000 units, hands a
traveller who has **never held this stablecoin** their first two swaps (creating their token
account for them), takes 50 units back out as cash, then counts the box, counts it short,
and shows the chain refusing the next swap.

```
  ok  initialize_kiosk #20261016
  kiosk PDA  FbKLdEC9SBJcpY8tfR3z83qK1xPyBHdmnqC5wHyFhynW  (bump 255), 180 bytes
  ok  fund treasury (1,000)
  ok  create traveller ATA #1        (the machine creates it — the traveller gave an address)
  ok  exchange #1 (100 -> 98)
  ok  create traveller ATA #2        (idempotent: a no-op the second time)
  ok  exchange #2 (100 -> 98)
  ok  redeem #3 (50 -> 49)           (the traveller signs; the machine owes them notes)
  ok  attest a full count
  ok  attest an empty box
  ok  swap #4 while frozen           refused: This machine is frozen ...

  traveller wallet   6WeT6JbiZUWZ2epahShzfpR4KFkHbYtev6giAgFQWR3
  their token acct   HS78BxmxKaW6BYJuG9R7VaNt6TwLnrR39vGY6ah4dB9a
    .owner (wallet)  6WeT6JbiZUWZ2epahShzfpR4KFkHbYtev6giAgFQWR3
    balance           146.00  (expected 146.00)
  kiosk treasury      854.00
  cash-out receipt   AoTavQifFBfgWJ1xZmZGQQXe7JMZezEmb9U31Asf868U
    .destination     6WeT6JbiZUWZ2epahShzfpR4KFkHbYtev6giAgFQWR3
    .gross/.net/.fee 50000000 / 49000000 / 1000000
    .kind            1  (1 = cash out)
  kiosk.total_swaps  3
  kiosk.last_serial  3
  kiosk.redeemed     49.00
  kiosk.frozen       True
```

Three real transactions, all finalized:
[kiosk creation](https://explorer.solana.com/tx/36PGJce7dpG3TEQFmViS8XLTzCAsuPZ5hDBQjNdxeProYYXiRioZUGzz1gF5TKgwruTistEJZBEMSVPXqvV7pWa2?cluster=devnet),
[the settlement that paid the traveller](https://explorer.solana.com/tx/34zugTA9Bj2UBEMccBgrKLZRjcRtXpWLEHK2AweAf9vqE5u7LLRuMUwqNy2YHGtvus7cjR1tiyXacfwAFBPVycLD?cluster=devnet)
and [the cash-out the traveller signed](https://explorer.solana.com/tx/3XZ1UwRtQV3ZRPZQDJ6xjm6NjGnPxCDjiXGkG5XvLzvWjxJPZTvu4PLyCRVGeH8KG2jBq8Qgj76RgfwFxJE4TpnK?cluster=devnet).

None of those numbers are cosmetic. `146.00` is the balance of the traveller's own token
account — 196 in from two swaps, 50 back out; `854.00` is what is left in the machine's
treasury; `frozen True` is the machine refusing further business because its box did not
add up. The UI is not in this loop.

```bash
# the whole product against devnet (needs a funded devnet keypair)
python scripts/devnet_smoke.py      # pip install solana solders
```

In mainland China, point it at a local RPC relay (`--rpc http://127.0.0.1:8899`): the public
devnet RPC is unreachable there, and the Solana websocket never honours a proxy.

---

## What is real, and what is simulated

This is a hackathon prototype, so being precise about the line matters more than
sounding big.

**Real**

- **The settlement program** — five instructions, 27 tests, deployed on devnet. It pays the
  traveller's own wallet, takes stablecoin back the other way, and freezes the machine when
  a physical count of its cash box does not add up. All of it executed for real in a devnet
  run ([evidence above](#deployed-on-devnet)).
- The whole interaction: a 3D kiosk, the UI **rendered onto the machine's screen as a
  texture** (so it cannot drift off the body), the banknotes as **draggable 3D
  objects** on a rack, and the five-step state machine.
- Currency conversion: `amount × rate × (1 − fee)`.
- **Live exchange rates**, fetched at runtime, with a **30-second quote lock** — the
  rate the traveller is shown is frozen and the settlement uses that frozen rate.
- **A real Solana devnet client**, written from scratch inside the file — no CDN, no
  `web3.js`, no wallet-adapter library. base58 codec, Ed25519 signing via WebCrypto,
  hand-assembled transaction, direct JSON-RPC.
- **The serialization is verified byte-for-byte against the official
  `@solana/web3.js`** — see [Verify](#verify). With a funded kiosk wallet the done
  screen shows a **real, confirmed devnet signature**.
- QR scanning for the wallet address uses the real camera when the browser allows it.

**Simulated — and labelled as such in the UI**

- Banknote intake: no hardware, no real note validation.
- **The browser demo does not call the program yet.** `atm.html` still sends a placeholder
  0.001 SOL transfer to a fixed demo destination — it was written before the program
  existed, and pointing it at the deployed program is roadmap item 1. We say so plainly,
  because it is the difference between a UI prototype and a working product; the settlement
  layer itself is real, and it is the layer that moves the money.
- **The UI has no cash-out screen yet.** The program takes stablecoin back in and the
  traveller can sign that transaction (the devnet run above does exactly that), but the
  kiosk screen only shows the cash-in flow so far.
- The demo stablecoin is a mint the smoke script creates (6 decimals, USDC-shaped). It is
  not real USDC.
- **The quote lock's rate movement is simulated.** The provider is a *daily*
  reference feed, so a live re-quote would return the same number and the lock would
  be invisible. A small random walk (±0.2% per step, capped at ±0.6%) is layered on
  top of the fetched rate and the screen says **「演示：模拟行情波动」**. Set
  `DEMO_FX_JITTER = false` and it reports 汇率未变 honestly instead of inventing
  movement. A production deployment would take a real quoting feed from its
  licensed partner and delete this.
- Wallet address validation checks that the string really base58-decodes to 32 bytes.
  It does not check that the address exists on chain.
- **No licence, no custodian, no KYC, no real money.** There is no regulated entity
  behind this demo.

If the devnet RPC or the kiosk wallet isn't available, the UI **falls back to a
simulated hash labelled `模拟`** — it never passes a fake hash off as a real one.

---

## Why no account and no phone number

The first version of this demo opened a **custodial account** keyed to a phone
number. We dropped it, for two reasons that both come from the actual user:

- **The target user is the worst case for SMS.** A traveller at an airport is roaming,
  on a foreign number, or has no signal — exactly the person you cannot send a
  verification code to. The machine's entire value is "done in two minutes".
- **For a handful of leftover notes, identity verification is disproportionate.** It
  also filters the wrong thing: an SMS proves you control *a* phone, not who you are,
  so it stops casual abuse while a burner SIM walks straight through.

So: **small amounts only**, no account, no sign-up, money to the traveller's own
wallet. Each currency carries a small-amount cap (placeholder values: JPY 10,000 /
USD 20 / EUR 50 / KRW 50,000 / THB 1,000), and the program enforces a per-machine ceiling
on chain, with cash out held to a quarter of it.

Two things worth saying out loud, because a judge will ask. First, refusing a phone number
is not refusing identity: if a jurisdiction demands "one person, one account", a
personhood proof (the World ID class of thing) does that job without an SMS, and it is the
route we would take rather than re-introducing the phone. Second, this is not a claim that
the design is risk-free — cash into a non-custodial wallet is precisely the pattern FATF
flags for money laundering. The caps, the per-machine limits and the fact that every
settlement is a public on-chain transaction are what make the exposure defensible; a
production launch would add sanctions screening on the destination address at the partner
layer.

---

## Note verification and ejection

Intake is not a denomination picker. Notes are physical objects on a rack; each one is
fed in and verified, and anything that fails is **physically ejected** — the note
travels back to its place on the rack — and counted on screen:

| Outcome | What happens |
| --- | --- |
| Genuine, within the cap | verified, added to the running total |
| Counterfeit | ejected, 验钞未通过 · 疑似假币 |
| Unrecognisable | ejected, 无法识别 |
| Would exceed the small-amount cap | ejected, 超出小额上限 |

(The demo's counterfeit and unrecognisable notes are tinted so you know which to pick.
A production cabinet gets this from a real note acceptor, and the state machine already
matches what one reports.)

---

## Exchange rates and the quote lock

Rates are fetched at runtime from <https://open.er-api.com> (CORS-open, no key). If
the network is missing, the response is malformed, or it stalls past 12 s, the kiosk
falls back to a built-in **ECB reference-rate snapshot** (2026-10-09) and says which
one it is using, on the kiosk screen and in the side panel.

The provider publishes a **daily** reference rate. It is not a tick feed, and the UI
labels it with the provider's rate date rather than calling it a live quote. That is
fine, because a kiosk does not need tick data — it needs a rate it can commit to:

> Choosing a currency **locks** the rate for 30 seconds. The conversion uses the locked
> value, so a rate change mid-flow cannot move the amount. When the window expires the
> quote is re-fetched, re-locked, and the change is shown (`已重新报价 1.1205 → 1.1193`).

Real bureaux de change work the same way: post a rate, hold it for a window. In
production the quoting feed — and the FX risk behind it — comes from the licensed
settlement partner, not from the machine.

---

## The client inside `atm.html`, in detail

This is the *UI's own* Solana layer, separate from the settlement program above.
`atm.html` contains a ~120-line Solana client. Why hand-rolled instead of a library?

- The product constraint was **one self-contained file that runs offline**. A CDN or a
  bundled `web3.js` breaks that.
- Signing uses the browser's native **Ed25519** (`crypto.subtle`), so no crypto
  library is needed either.
- The transaction is an ordinary `SystemProgram.transfer`: header `[1,0,1]`, three
  account keys (payer, destination, SystemProgram), 12-byte transfer instruction.

The **destination is the address the traveller provided** — decoded and checked to be
32 bytes before signing. The kiosk's own wallet only pays the fee; it is a
browser-generated keypair persisted in `localStorage`, devnet-only and worthless.

---

## Getting a real on-chain transaction

Public devnet faucets are rate-limited and frequently dry, so the faucet is *not* part
of the happy path:

1. Open `atm.html`. The right-hand 钱包 card shows the **kiosk wallet** address.
2. Click **复制本机地址**, then fund it once at <https://faucet.solana.com> (devnet, ~1 SOL).
3. Reload and run the flow. Each exchange now sends a real devnet transaction; the done
   screen switches from `SOLANA · 模拟` to `SOLANA · DEVNET / 已确认`.

Don't clear browser storage or move the file — a new path means a new keypair.

---

## Verify

```bash
npm i
npm test          # runs verify.js
```

```
== 1. 语法（V8 解析发布文件里的 app JS）==   PASS
== 2. 文案禁用词（应为 0）==                  PASS  (all 5 terms = 0)
== 3. 手写序列化 vs 官方库（直接取文件里的函数）==
  PASS b58enc 与官方 PublicKey.toBase58 一致
  PASS message 与官方 serializeMessage() 逐字节一致
  PASS 整条交易与官方 serialize() 逐字节一致
  PASS 官方库能解析我拼的交易且验签通过
```

The checker doesn't compare against a copy — it runs the app JS from `atm.html` in a
VM and pulls the *actual* shipped functions out, so it can't drift from the source.

Check 2 is the **copy-compliance guard**, and it is the one test that enforces product
copy rather than code. The interface deliberately avoids crypto jargon a traveller
would not understand (私钥 / 助记词 / 链上 / 代币 / gas never appear); the Solana layer
is real and explainable, it just isn't shouted at the customer.

The program has its own suite — `cargo test`, 17 tests, run against the real SPL Token
program inside LiteSVM ([above](#the-settlement-layer-on-chain)).

---

## Repo layout

```
programs/changelink/src/        the settlement program (Anchor/Rust)
  lib.rs                        the five instructions
  state/{kiosk.rs,receipt.rs}   the two account types
  instructions/                 initialize_kiosk · exchange · redeem · set_limits
                                · withdraw_treasury · attest_backing
programs/changelink/tests/      LiteSVM tests — real .so, real SPL Token program
scripts/devnet_smoke.py         end-to-end run against devnet
Anchor.toml · Cargo.toml        build config; the program id lives in lib.rs and Anchor.toml
atm.html                        the kiosk UI — single file, self-contained
verify.js                       verification harness (syntax, copy guard, official-lib diff)
package.json                    npm test → verify.js
演示讲稿.md                      the 3-minute demo script (Chinese)
deck/零钱兑换机_逻辑链.pdf         logic-chain deck (PDF)
deck/零钱兑换机_逻辑链.pptx        logic-chain deck (PPTX source)
```

## Who pays

The traveller pays a **2% exchange fee** (demo constant) — below the 3–10% a bureau de
change charges. The kiosk takes airport/venue floor space, so the venue takes a
revenue share, and the settlement flow earns the partner the spread. Hardware is a
known cost, not a bet: a screen, a note acceptor and a network connection.

---

## Roadmap

1. **Point `atm.html` at the deployed program** — swap the placeholder SOL transfer for the
   `exchange` instruction, and create the traveller's token account in the same transaction
   (`CreateIdempotent`, the machine pays the rent). That layout is already proven on devnet
   by `scripts/devnet_smoke.py`; only the UI is behind.
2. **A cash-out screen in the UI.** The program supports it and the traveller signs it —
   the devnet run above does it — but the kiosk screen only shows the cash-in flow.
3. **Real USDC** — the devnet run uses a demo mint; moving to USDC is a mint address and a
   treasury funding step, not a program change.
4. **A licensed settlement partner** for quoting, screening and any KYC the
   jurisdiction requires — the product is designed so this is a partner integration,
   not a rewrite.
5. **Note-acceptor hardware** — the current intake is on-screen; the state machine
   already matches what a BV/CPI acceptor reports.
6. **A device-generated key for receipts** — the machine signs what it counted, with the
   public key stored in the `Kiosk` account and checked by the program, so a cloned machine
   cannot mint value. Stated carefully: Takarabako already ships the device-signed note with
   the key published in ENS, so this is only worth doing if we can say something more than
   "we sign it too" — our only angle is that the check happens in the program that pays, with
   no resolver in the path.

---

## Things that bit us

Worth writing down, so nobody on this team repeats them:

- **`anchor build` produced a binary devnet would not accept**
  (`ELF error: Detected sbpf_version required by the executable which are not enabled`).
  The CLI pulls the newest platform-tools; the deployable artifact comes from
  `cargo build-sbf` (default `--arch v0`).
- **anchor-cli version pinning**: `anchor-lang = 1.2.0` resolves to the newest 1.2.x and the
  CLI then refuses to build, because it wants that exact CLI present. It has to be `=1.2.0`.
- **A traveller who has never held the stablecoin has no token account**, so the settlement
  would fail on the last screen of the demo. The machine creates it, idempotently, in the
  same transaction — and pays the rent (about 0.002 SOL, well inside a 2% fee).
- **LiteSVM's blockhash never changes**: two identical transactions hash to the same
  signature and the second comes back `AlreadyProcessed` rather than the program's own error,
  which reads like a broken test. `svm.expire_blockhash()` before each send.
- **Devnet's public RPC is load-balanced**, so a just-confirmed account can still read as
  missing. Poll for it instead of believing the first `None`.
- **From mainland China the public devnet RPC is unreachable and the Solana websocket ignores
  proxies** — a local relay that forwards both HTTP and WS is the only thing that works
  (`--rpc http://127.0.0.1:8899`).

---

## Development disclosure

Per Colosseum's rules, all relevant development history is disclosed:

- **All application code in this repository was written on 2026-10-11**, inside the
  Crypto World's Fair window — the kiosk UI and the on-chain program alike. There is no
  pre-existing product codebase behind it.
- **Development tools:** the code was typed substantially by AI coding agents (Claude Code,
  Hermes) under our direction — the design decisions, the limits, the invariants and the
  refusals above are ours, and we can defend each one without notes. Every claim in this
  README is backed by a command you can re-run: `cargo test`, `npm test`,
  `python scripts/devnet_smoke.py`.
- **Third-party code:** three.js r128 (MIT) is inlined into `atm.html`. `anchor-lang` and
  `anchor-spl` are the program's only dependencies. `@solana/web3.js` is a dev-only
  dependency used by `verify.js` as a reference implementation; it is not shipped in the
  product.
- **External services at runtime:** `api.devnet.solana.com` (settlement) and
  `open.er-api.com` (exchange rates). Both degrade to a labelled offline fallback.
- **Prior work:** market research on the currency-exchange kiosk space (including
  reading TravelersBox's own documentation) informed the positioning. No code from it.

## License

MIT — see [LICENSE](LICENSE).
