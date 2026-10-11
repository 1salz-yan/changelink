# ChangeLink — leftover foreign cash, straight into your own wallet, on Solana

A self-service kiosk for the foreign coins and banknotes a traveller can't spend
any more. Drop in the notes, scan your own wallet's receive code, and the same
value arrives as **USDC in your wallet** — no sign-up, no account, no phone number.

**Single file · no build step · no framework.** Open `atm.html` in any modern
browser. That's the whole product.

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

---

## Try it

```
open atm.html          # macOS — or just double-click it
```

No server, no install. three.js **is inlined into the file** (r128, MIT), so the 3D
kiosk renders offline.

The flow is five steps:

1. **选币种** — pick the currency you're holding.
2. **放钞** — drag a banknote from the rack into the intake slot. The machine
   verifies each note.
3. **收款钱包** — type your wallet's receive address on the on-screen keyboard
   (the alphabet is base58 only, so there is no `0/O/I/l` to mistype), or scan the QR.
4. **兑换** — the machine converts and sends.
5. **完成** — amount, the locked rate, and the transaction hash.

---

## What is real, and what is simulated

This is a hackathon prototype, so being precise about the line matters more than
sounding big.

**Real**

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
- **The quote lock's rate movement is simulated.** The provider is a *daily*
  reference feed, so a live re-quote would return the same number and the lock would
  be invisible. A small random walk (±0.2% per step, capped at ±0.6%) is layered on
  top of the fetched rate and the screen says **「演示：模拟行情波动」**. Set
  `DEMO_FX_JITTER = false` and it reports 汇率未变 honestly instead of inventing
  movement. A production deployment would take a real quoting feed from its
  licensed partner and delete this.
- Wallet address validation checks that the string really base58-decodes to 32 bytes.
  It does not check that the address exists on chain.
- **Settlement is demoed as a 0.001 SOL transfer, not an SPL stablecoin transfer.**
  Devnet USDC, associated token accounts and a funded treasury are the next step. The
  product copy says USDC because that is the product intent; the on-chain layer is
  labelled truthfully as a devnet transfer.
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
USD 20 / EUR 50 / KRW 50,000 / THB 1,000). This is not a claim that the design is
risk-free — cash into a non-custodial wallet is precisely the pattern FATF flags for
money laundering. The cap, the per-machine limits and the fact that every settlement
is a public on-chain transaction are what make the exposure defensible; a production
launch would add sanctions screening on the destination address at the partner layer.

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

## Solana integration, in detail

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

---

## Repo layout

```
atm.html                        the product — single file, self-contained
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

1. **SPL stablecoin settlement on devnet** — real USDC instead of a SOL transfer, plus
   a treasury/hot-wallet layout that looks like production.
2. **A licensed settlement partner** for quoting, screening and any KYC the
   jurisdiction requires — the product is designed so this is a partner integration,
   not a rewrite.
3. **Note-acceptor hardware** — the current intake is on-screen; the state machine
   already matches what a BV/CPI acceptor reports.

---

## Development disclosure

Per Colosseum's rules, all relevant development history is disclosed:

- **All application code in this repository was written on 2026-10-11**, inside the
  Crypto World's Fair window. There is no pre-existing product codebase behind it.
- **Third-party code:** three.js r128 (MIT) is inlined into `atm.html` and is the only
  third-party dependency in the product. `@solana/web3.js` is a dev-only dependency
  used by `verify.js` as a reference implementation; it is not shipped in the product.
- **External services at runtime:** `api.devnet.solana.com` (settlement) and
  `open.er-api.com` (exchange rates). Both degrade to a labelled offline fallback.
- **Prior work:** market research on the currency-exchange kiosk space (including
  reading TravelersBox's own documentation) informed the positioning. No code from it.

## License

MIT — see [LICENSE](LICENSE).
