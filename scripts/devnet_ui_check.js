#!/usr/bin/env node
/* 端到端：把 atm.html 里**发布的那份 app JS** 拿到 Node 里跑，连真 devnet。
   走的是同一个函数入口（solInitKiosk / solSendExchange），同一套 WebCrypto ed25519，
   同一套手拼字节 —— 只有 DOM/画布那层是桩。所以它证的是"发布代码能上链"。
   localStorage 用内存桩（里面放一把测试用的机台钱包，只在本机备份里）。

   用法: [RELAY=http://127.0.0.1:8899] node scripts/devnet_ui_check.js
   前置: 先跑 scripts/provision_kiosk.py --wallet <上面那把钱包的地址>       */
const fs = require("fs"), path = require("path"), vm = require("vm"), crypto = require("node:crypto");

const HTML = path.join(__dirname, "..", "atm.html");
// 机台钱包的私钥只在本机（不进仓库）：{p: base64(pkcs8), pub: base64(raw)}，
// 跟浏览器 localStorage 里那份同格式。用 KEYFILE=... 指定别处。
const KEYFILE = process.env.KEYFILE || path.join(process.env.HOME, ".hermes", "backups", "browser_kiosk_test.json");
const RELAY = process.env.RELAY || "http://127.0.0.1:8899";
const TRAVELLER = process.env.TRAVELLER || "GTmwnfKmSKVdE2KE5yVNuhWU7qEJtMcR8HM4Q1TRDRrW";
const GROSS = BigInt(process.env.GROSS || 10000000);   // 10 稳定币

let fail = 0;
const ok = m => console.log("  \x1b[32mPASS\x1b[0m " + m);
const no = m => { console.log("  \x1b[31mFAIL\x1b[0m " + m); fail = 1; };

const app = [...fs.readFileSync(HTML, "utf8").matchAll(/<script>([\s\S]*?)<\/script>/g)]
  .map(m => m[1]).pop();

const any = new Proxy(function () {}, {
  get: (_t, k) => k === Symbol.toPrimitive ? (() => 0) : k === "valueOf" ? (() => 0)
                 : k === "toString" ? (() => "") : any,
  set: () => true, apply: () => any, construct: () => any,
});

const store = {};
const seeded = JSON.parse(fs.readFileSync(KEYFILE, "utf8"));
store["cl_sol_key"] = JSON.stringify({ p: seeded.p, pub: seeded.pub });

const sleep = ms => new Promise(r => setTimeout(r, ms));
const rpcOnce = (method, params) => fetch(RELAY, {
  method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
}).then(r => r.json()).then(j => { if (j.error) throw new Error(method + ": " + JSON.stringify(j.error)); return j.result; });

/* 公共 devnet + 本地中继都会偶发抽一下 —— 退避重试，别让一次抖动判死刑 */
const rpc = async (method, params, attempts = 5) => {
  let last;
  for (let i = 0; i < attempts; i++) {
    try { return await rpcOnce(method, params); }
    catch (e) { last = e; await sleep(1500 * (i + 1)); }
  }
  throw last;
};

(async function () {
  const sb = {
    console, Math, Date, JSON, Promise, setTimeout, clearTimeout, setInterval, clearInterval,
    btoa, atob, Uint8Array, Uint32Array, TextEncoder, TextDecoder, BigInt,
    document: any, window: any, navigator: any,
    localStorage: { getItem: k => (k in store ? store[k] : null), setItem: (k, v) => { store[k] = String(v); }, removeItem: k => { delete store[k]; } },
    requestAnimationFrame: () => 0, cancelAnimationFrame: () => {},
    performance: { now: () => Date.now() },
    crypto: crypto.webcrypto, fetch: () => Promise.reject(new Error("offline in this harness")),
  };
  sb.globalThis = sb;
  vm.createContext(sb);
  try { vm.runInContext(app, sb); } catch (e) { console.log("    · vm 预执行中断(可忽略): " + e.message); }

  // 让发布代码去说本地中继的话（浏览器里它直连 api.devnet.solana.com）
  sb.SOL_RPC = RELAY;
  sb.solRpc = rpc;                                  // 同一个调用口径，只是送到中继
  if (typeof sb.solInitKiosk !== "function" || typeof sb.solSendExchange !== "function")
    throw new Error("没能取出 solInitKiosk / solSendExchange");
  if (!sb.CL || !sb.CL.kiosk) throw new Error("CL 还没被 provision_kiosk.py 填过");

  console.log(`RPC ${RELAY}   kiosk ${sb.CL.kiosk}`);
  console.log(`traveller ${TRAVELLER}   gross ${GROSS} (1e-6)\n`);

  console.log("== 1. 机台自己的钱包签名，把自己注册上链 ==");
  let initSig = "";
  // 刚确认的账户可能还读不到（公共 RPC 是负载均衡的）—— 轮询，别信第一次的 null
  const kioskAcc = async () => {
    for (let i = 0; i < 20; i++) {
      const v = (await rpc("getAccountInfo", [sb.CL.kiosk, { encoding: "base64" }])).value;
      if (v) return v;
      await sleep(1500);
    }
    return null;
  };
  const already = await kioskAcc();
  if (already && already.owner === sb.CL.prog) {
    ok(`这台机器已经在链上注册过了（owner = 程序，${sb.CL.kiosk.slice(0, 8)}…）`);
  } else {
    try {
      const rec = await sb.solInitKiosk();
      initSig = String(rec.sig);
      const acc = await kioskAcc();
      acc && acc.owner === sb.CL.prog
        ? ok(`initialize_kiosk 上链成功，config 账户已存在（${initSig.slice(0, 12)}…）`)
        : no("initialize_kiosk 发出去但账户没建起来");
    } catch (e) {
      no("initialize_kiosk 失败: " + String(e.message || e).slice(0, 160));
    }
  }

  console.log("\n== 2. 旅客只给地址，机器结算 ==");
  // 旅客的账户是**累加**的（他不止来一次），所以要比的是这一笔的增量
  const ataB58 = sb.b58enc(await sb.ataOf(TRAVELLER, sb.CL.mint));
  const balanceOf = async addr => {
    try { return Number((await rpc("getTokenAccountBalance", [addr])).value.amount); }
    catch (e) { return 0; }     // 账户还不存在 = 0
  };
  const before = await balanceOf(ataB58);

  let swapSig = "";
  try {
    const rec = await sb.solSendExchange(TRAVELLER, GROSS);
    swapSig = String(rec.sig);
    ok(`exchange 上链成功，confirmed=${rec.confirmed}（${swapSig.slice(0, 12)}…）`);
  } catch (e) {
    no("exchange 失败: " + String(e.message || e).slice(0, 200));
  }

  console.log("\n== 3. 链上核验这笔钱真的到了旅客名下的账户 ==");
  if (swapSig) {
    const r = await rpc("getSignatureStatuses", [[swapSig], { searchTransactionHistory: true }]);
    const st = r.value && r.value[0];
    st && !st.err
      ? ok(`节点认这笔交易（status=${st.confirmationStatus || "confirmed"}）`)
      : no("节点不认这笔交易: " + JSON.stringify(st));

    const after = await balanceOf(ataB58);
    const delta = (after - before) / 1e6;
    const want = Number(GROSS - GROSS * 200n / 10000n) / 1e6;
    console.log(`    · 旅客的 token account ${ataB58}（地址由发布代码自己推导）`);
    delta === want
      ? ok(`这一笔到账 ${delta} 稳定币 = 毛额扣 2% 手续费（余额 ${before / 1e6} → ${after / 1e6}）`)
      : no(`这一笔到账 ${delta}，应该是 ${want}`);
  }

  console.log("\n  签名: https://explorer.solana.com/tx/" + (swapSig || "") + "?cluster=devnet");
  console.log(fail ? "\n\x1b[31m❌ 有失败项\x1b[0m" : "\n\x1b[32m✅ 全部通过\x1b[0m");
  process.exit(fail);
})().catch(e => { console.log("  \x1b[31mFAIL\x1b[0m " + (e && e.message)); process.exit(1); });
