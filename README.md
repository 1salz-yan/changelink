# ChangeLink — turn leftover foreign cash into stablecoin, on Solana

A self-service kiosk that lets a traveller drop in the foreign banknotes they can't use
and walk away with the same value as stablecoin in a **custodial exchange account** they
can withdraw to their own wallet at any time.

**Single file · no build step · no framework · works offline.** Open `atm.html` in any
modern browser. That's the whole product.

Built for Colosseum **Crypto World's Fair** (Oct 2026).

---

## The problem

Every traveller ends up with a handful of foreign banknotes and coins. Bureaux de change
mostly won't buy coins at all, the ones that do charge 3–10%, and whatever's left is
carried home to rot in a drawer. The money is already spent — it's just stranded.

**Prior art, stated honestly:** [TravelersBox](https://www.travelersbox.com/) has run
coin/kiosk exchange machines at airports since 2012 and is still operating. We're not
claiming an empty field. The contrast is the settlement layer and the exit: TravelersBox
pays out gift cards and donations; we settle the value **on Solana** into an account the
traveller can actually withdraw from.

---

## Try it

```
open atm.html          # macOS — or just double-click it
```

No server, no install, no network required to run the flow. The 3D kiosk is built with
three.js **inlined into the file** (r128, MIT), so it renders offline.

Walk the flow: **开始兑换** → insert notes → **完成放钞** → phone + code → processing →
done (with a transaction hash) → optional **withdraw to your own wallet**.

---

## What is real, and what is simulated

This is a hackathon prototype. Being precise about the line matters more than sounding big.

**Real**

- The whole interaction: 3D kiosk, projected UI on the machine screen, the state machine
  (`放钞 → 开账户 → 兑换 → 完成` + optional withdrawal), the amount math
  `amount × rate × (1 − fee)`.
- **A real Solana devnet client**, written from scratch in the file — no CDN, no
  `web3.js`, no wallet-adapter library. base58 codec, Ed25519 signing via WebCrypto,
  hand-assembled transaction, direct JSON-RPC to `api.devnet.solana.com`.
- **The serialization is verified byte-for-byte against the official
  `@solana/web3.js`** — see [Verify](#verify). When the demo account is funded, the done
  screen shows a **real, confirmed devnet signature** with an explorer link.
- QR scanning for the withdrawal step uses the real camera (`getUserMedia` +
  `BarcodeDetector`) when the browser grants it.

**Simulated (clearly labelled as such in the UI)**

- Banknote intake — no hardware, no note validation.
- Phone number and verification code — no SMS; the code is fixed at `123456` for demo.
- Identity verification / KYC — no provider is connected.
- **The custodian, the licence, and the funds** — there is no licensed partner behind
  this demo, and no real money anywhere in it.
- Exchange rate (fixed 1.00) and fee (2%) are demo constants.
- **Settlement is demoed as a 0.001 SOL transfer, not an SPL stablecoin transfer.**
  Devnet USDC, associated token accounts and a funded treasury are the next step. We say
  "stablecoin" in the product copy because that's the product intent, and we label the
  on-chain layer truthfully as a devnet transfer.

If the devnet RPC or the demo account isn't available, the UI **falls back to a
simulated hash and labels it `模拟`** — it never passes a fake hash off as a real one.

---

## Solana integration, in detail

`atm.html` contains a ~120-line Solana client. Why hand-rolled instead of a library?

- The product constraint was **one self-contained file that runs offline**. A CDN or a
  bundled `web3.js` breaks that.
- Signing uses the browser's native **Ed25519** (`crypto.subtle`), so no crypto library
  is needed either.
- The transaction is an ordinary `SystemProgram.transfer`: header `[1,0,1]`, three
  account keys (payer, destination, SystemProgram), 12-byte transfer instruction.

The demo account is a browser-generated keypair, persisted in `localStorage` so repeated
runs reuse the same funded account. It is a **devnet-only demo key with no value**.

---

## Getting a real on-chain transaction

Public devnet faucets are rate-limited and frequently dry, so the faucet is *not* part of
the happy path:

1. Open `atm.html`. The right-hand card shows the **Solana account** address.
2. Click **复制地址**, then fund it once at <https://faucet.solana.com> (devnet, ~1 SOL).
3. Reload and run the flow. Each exchange now sends a real devnet transaction; the done
   screen switches from `SOLANA · 模拟` to `SOLANA · DEVNET / 已确认` and exposes an
   explorer link.

Don't clear browser storage or move the file — a new path means a new keypair.

---

## Verify

```bash
npm i @solana/web3.js@1     # only used as the reference implementation
node verify.js
```

```
== 1. 语法（V8 解析发布文件里的 app JS）==   PASS
== 2. 文案禁用词（应为 0）==                  PASS  (all 7 terms = 0)
== 3. 手写序列化 vs 官方库（直接取文件里的函数）==
  PASS b58enc 与官方 PublicKey.toBase58 一致
  PASS message 与官方 serializeMessage() 逐字节一致
  PASS 整条交易与官方 serialize() 逐字节一致
  PASS 官方库能解析我拼的交易且验签通过
```

The checker doesn't compare against a copy — it runs the app JS from `atm.html` in a VM
and pulls the *actual* shipped functions out, so it can't drift from the source.

Check 2 is the **copy-compliance guard**. The product deliberately uses custodial,
regulated vocabulary (账户 / 余额 / 手机号 / 验证码) and bans crypto jargon
(私钥 / 助记词 / seed / gas / 链上 / USDC) from the interface, because the traveller
standing at the machine is not a crypto user. The Solana layer is real and explainable —
it just isn't shouted at the customer.

---

## Repo layout

```
atm.html                        the product — single file, self-contained
verify.js                       verification harness (syntax, copy guard, official-lib diff)
演示讲稿.md                       the 3-minute demo script (Chinese)
deck/零钱兑换机_逻辑链.pdf         logic-chain deck (PDF)
deck/零钱兑换机_逻辑链.pptx        logic-chain deck (PPTX source)
```

---

## Who pays

The traveller pays a **2% exchange fee** on the amount converted (demo constant) — below
the 3–10% a bureau de change charges, and below what a bank charges on cash exchange.
The kiosk sits on airport/venue floor space, so the venue takes a revenue share, and the
custodial balance generates float income for the licensed partner. Hardware is a known
cost, not a bet: the unit is a screen, a note acceptor and a network connection.

---

## Roadmap

1. **SPL stablecoin settlement on devnet** — real USDC transfer instead of a SOL transfer,
   plus a treasury/hot-wallet layout that looks like production.
2. **A licensed custodial partner** for the account and KYC layer — the product is
   designed so this is a partner integration, not a rewrite.
3. **Note-acceptor hardware** — the current intake is on-screen; the state machine already
   matches what a BV/CPI acceptor reports.

---

## Development disclosure

Per Colosseum's rules, all relevant development history is disclosed:

- **All application code in this repository was written on 2026-10-11**, inside the
  Crypto World's Fair window. There is no pre-existing product codebase behind it.
- **third-party code:** three.js r128 (MIT) is inlined into `atm.html` and is the only
  third-party dependency in the product. `@solana/web3.js` is a dev-only dependency used
  by `verify.js` as a reference implementation; it is not shipped in the product.
- **Prior work:** market research on the currency-exchange kiosk space (including
  reading TravelersBox's own documentation) informed the positioning. No code from it.

## License

MIT — see [LICENSE](LICENSE).
