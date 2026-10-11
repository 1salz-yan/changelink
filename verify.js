#!/usr/bin/env node
/* 一键校验 atm.html（对“发布的那份文件”本身验证，不是副本）
   1) 内联 app JS 语法   2) 文案禁用词   3) 手写 Solana 序列化 vs 官方 @solana/web3.js
   用法: node verify.js     依赖(可选): @solana/web3.js@1 —— 缺了就自动 SKIP 第 3 项 */
const fs = require('fs'), path = require('path'), vm = require('vm');

const HTML = path.join(__dirname, 'atm.html');
const src = fs.readFileSync(HTML, 'utf8');
let fail = 0;
const ok = m => console.log('  \x1b[32mPASS\x1b[0m ' + m);
const no = m => { console.log('  \x1b[31mFAIL\x1b[0m ' + m); fail = 1; };

/* ---- 抽出最后一个 <script>（app JS；前一个是内联的 three.js）---- */
const blocks = [...src.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(m => m[1]);
const app = blocks[blocks.length - 1];

console.log('== 1. 语法（V8 解析发布文件里的 app JS）==');
try { new Function(app); ok(`JS 语法，${app.length} 字符，另有 ${blocks.length - 1} 个内联引擎块`); }
catch (e) { no('JS 语法: ' + e.message); }

/* 禁用词：产品不再隐藏加密层，USDC 与「钱包地址」已按需求解禁（2026-10-11） */
console.log('== 2. 文案禁用词（应为 0）==');
for (const w of ['私钥', '助记词', '链上', '代币', 'gas'])
  (src.match(new RegExp(w, 'gi')) || []).length === 0 ? ok(`${w} = 0`) : no(`${w} 出现 ${(src.match(new RegExp(w, 'gi')) || []).length} 次`);

console.log('== 3. 手写序列化 vs 官方库（直接取文件里的函数）==');
try {
  const web3 = require('@solana/web3.js');
  const { Keypair, Transaction, SystemProgram, PublicKey } = web3;

  /* 用宽松代理当 DOM 桩；显式给 valueOf/toString/toPrimitive，否则 V8 会在做算术时抛错 */
  const any = new Proxy(function () {}, {
    get: (_t, k) => k === Symbol.toPrimitive ? (() => 0)
                   : k === 'valueOf'        ? (() => 0)
                   : k === 'toString'       ? (() => '')
                   : any,
    set: () => true,
    apply: () => any,
    construct: () => any,
  });
  const sb = { console, Math, Date, JSON, Promise, setTimeout, clearTimeout, setInterval, clearInterval,
    btoa, atob, Uint8Array, Uint32Array, TextEncoder, document: any, window: any, localStorage: any,
    navigator: any, crypto: { subtle: any }, fetch: any };
  sb.globalThis = sb;
  vm.createContext(sb);
  try { vm.runInContext(app, sb); } catch (e) { console.log('    · vm 预执行中断(可忽略): ' + e.message); }

  const { b58enc, vshort, u32le, u64le, uconcat, SOL_DEST, SOL_LAMPORTS } = sb;
  if (![b58enc, vshort, u32le, u64le, uconcat].every(f => typeof f === 'function'))
    throw new Error('未能从 atm.html 取出序列化函数');

  const from = Keypair.generate();
  console.log('    · web3.js', require('@solana/web3.js/package.json').version,
    '| helpers:', ['b58enc', 'vshort', 'u32le', 'u64le', 'uconcat'].map(k => typeof sb[k]).join('/'),
    '| SOL_DEST:', sb.SOL_DEST && sb.SOL_DEST.length, '| SOL_LAMPORTS:', sb.SOL_LAMPORTS);
  b58enc(new Uint8Array(from.publicKey.toBytes())) === from.publicKey.toBase58()
    ? ok('b58enc 与官方 PublicKey.toBase58 一致') : no('b58enc 与官方不一致');

  const bhBytes = Buffer.from(Array.from({ length: 32 }, (_, i) => 255 - i));
  const tx = new Transaction({ recentBlockhash: b58enc(new Uint8Array(bhBytes)), feePayer: from.publicKey });
  tx.add(SystemProgram.transfer({ fromPubkey: from.publicKey, toPubkey: new PublicKey(Buffer.from(SOL_DEST)), lamports: SOL_LAMPORTS }));

  const sys = new Uint8Array(32);
  const myMsg = Buffer.from(uconcat([
    new Uint8Array([1, 0, 1]),
    new Uint8Array(vshort(3)), new Uint8Array(from.publicKey.toBytes()), new Uint8Array(SOL_DEST), sys, new Uint8Array(bhBytes),
    new Uint8Array(vshort(1)),
    new Uint8Array([2]),
    new Uint8Array(vshort(2)), new Uint8Array([0, 1]),
    new Uint8Array(vshort(12)), u32le(2), u64le(SOL_LAMPORTS),
  ]));
  Buffer.compare(myMsg, tx.serializeMessage()) === 0 ? ok('message 与官方 serializeMessage() 逐字节一致') : no('message 与官方不一致');

  tx.sign(from);
  const myTx = Buffer.concat([Buffer.from([1]), Buffer.from(tx.signature), myMsg]);
  Buffer.compare(myTx, tx.serialize()) === 0 ? ok('整条交易与官方 serialize() 逐字节一致') : no('整条交易与官方不一致');
  Transaction.from(myTx).verifySignatures() ? ok('官方库能解析我拼的交易且验签通过') : no('验签失败');
} catch (e) {
  if (e.code === 'MODULE_NOT_FOUND')
    console.log('  \x1b[33mSKIP\x1b[0m 未装 @solana/web3.js（安装: npm i @solana/web3.js@1）');
  else no('序列化校验异常: ' + e.message);
}

console.log(fail ? '\n\x1b[31m❌ 有失败项\x1b[0m' : '\n\x1b[32m✅ 全部通过\x1b[0m');
process.exit(fail);
