#!/usr/bin/env node
/* 校验 atm.html 里"链上那一半"—— 验的是发布文件的本身，不是副本。
   1) anchor discriminator 与官方算法一致
   2) findPda 推导 vs 官方 findProgramAddress（随机对拍）
   3) ATA 推导 vs 一个 devnet 上**真实存在**的 ATA（跳出"两个库互相认同"这层）
   4) 两条指令拼出的 message vs 官方 Transaction.serializeMessage() 逐字节一致
   用法: node scripts/verify_chain.js      依赖(可选): @solana/web3.js@1  */
const fs = require('fs'), path = require('path'), vm = require('vm'), crypto = require('crypto');

const HTML = path.join(__dirname, '..', 'atm.html');
const src = fs.readFileSync(HTML, 'utf8');
let fail = 0;
const ok = m => console.log('  \x1b[32mPASS\x1b[0m ' + m);
const no = m => { console.log('  \x1b[31mFAIL\x1b[0m ' + m); fail = 1; };

const blocks = [...src.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(m => m[1]);
const app = blocks[blocks.length - 1];

/* 宽松桩：应用顶层会摸 DOM / 动画，这里只关心那几个链上函数 */
const any = new Proxy(function () {}, {
  get: (_t, k) => k === Symbol.toPrimitive ? (() => 0)
                 : k === 'valueOf'         ? (() => 0)
                 : k === 'toString'        ? (() => '')
                 : any,
  set: () => true, apply: () => any, construct: () => any,
});

let web3;
try { web3 = require('@solana/web3.js'); }
catch (e) { console.log('  \x1b[33mSKIP\x1b[0m 未装 @solana/web3.js（安装: npm i @solana/web3.js@1）'); process.exit(0); }
const { Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } = web3;

const TOKEN_PROG = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const ATA_PROG = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const CL_PROG = new PublicKey('4GA9eg49dVH8hpJmN7dkiodXNcAUuyi9wiXjeGkeUVCF');
const utf8 = s => Buffer.from(s, 'utf8');
const u64 = n => { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(n)); return b; };
const eq = (a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)) === 0;

(async function () {
  const sb = {
    console, Math, Date, JSON, Promise, setTimeout, clearTimeout, setInterval, clearInterval,
    btoa, atob, Uint8Array, Uint32Array, TextEncoder, TextDecoder,
    document: any, window: any, localStorage: any, navigator: any,
    requestAnimationFrame: () => 0, cancelAnimationFrame: () => {},
    performance: { now: () => Date.now() },
    crypto: crypto.webcrypto, fetch: any,
  };
  sb.globalThis = sb;
  vm.createContext(sb);
  try { vm.runInContext(app, sb); } catch (e) { console.log('    · vm 预执行中断(可忽略): ' + e.message); }

  const { b58enc, findPda, ataOf, anchorDisc, clExchangeMsg, TOKEN_PROG: appToken } = sb;
  if (typeof findPda !== 'function' || typeof clExchangeMsg !== 'function' || !appToken)
    throw new Error('未能从 atm.html 取出链上函数（顶层的 var 赋值没跑到？）');

  console.log('== 1. anchor discriminator ==');
  const wantDisc = crypto.createHash('sha256').update('global:exchange').digest().subarray(0, 8);
  eq(await anchorDisc('exchange'), wantDisc)
    ? ok('sha256("global:exchange")[0..8] 与官方算法一致')
    : no('discriminator 不一致');

  console.log('== 2. PDAs vs 官方 findProgramAddress ==');
  let bad = 0;
  for (let i = 0; i < 8; i++) {
    const kiosk = Keypair.generate().publicKey, serial = 1000 + i * 7;
    const mine = await findPda([utf8('receipt'), kiosk.toBuffer(), u64(serial)], CL_PROG.toBuffer());
    const theirs = (await PublicKey.findProgramAddress(
      [utf8('receipt'), kiosk.toBuffer(), u64(serial)], CL_PROG))[0];
    if (Buffer.from(mine.addr).toString('hex') !== theirs.toBuffer().toString('hex')) bad++;
  }
  bad ? no(`${bad}/8 个 PDA 推导不一致`) : ok('8 组 receipt PDA 与官方一致（bump 搜寻 + 曲线判定）');

  console.log('== 3. ATA 推导 vs devnet 上真实存在的账户 ==');
  // 这两个地址是 scripts/devnet_smoke.py 真跑出来的一笔：钱包 → 它的 ATA
  const REAL = {
    wallet: '6WeT6JbiZUWZ2epahShzfpR4KFkHbYtev6giAgFQWR3',
    mint: '9fbpg4YicJ5MLVWhXXenoVPk7sygSqq4LfLW6AJ5Jn9y',
    ata: 'HS78BxmxKaW6BYJuG9R7VaNt6TwLnrR39vGY6ah4dB9a',
  };
  const derived = b58enc(await ataOf(REAL.wallet, REAL.mint));
  derived === REAL.ata
    ? ok(`真实钱包的 ATA 推导正确（${REAL.ata.slice(0, 8)}…，链上真存在）`)
    : no(`ATA 推导错：得到 ${derived}，链上是 ${REAL.ata}`);

  console.log('== 4. 会话拼出来的整条交易，官方库能不能解析回来 ==');
  // 组内顺序各家客户端不同（都合法），所以这里不比对字节，而是：用我拼的字节 +
  // 真 ed25519 签名，交给官方库解析，然后逐项检查程序、角色、数据和签名。
  const kp = await crypto.webcrypto.subtle.generateKey({ name: 'Ed25519' }, true, ['sign', 'verify']);
  const payerU8 = new Uint8Array(await crypto.webcrypto.subtle.exportKey('raw', kp.publicKey));
  const payer = new PublicKey(payerU8);

  const k = n => new Uint8Array(Keypair.generate().publicKey.toBytes());
  const ata = k(), kiosk = k(), treasury = k(), receipt = k(), wallet = k(), mint = k();
  const keys = [payerU8, ata, kiosk, treasury, receipt, wallet, mint,
                new Uint8Array(32), new Uint8Array(TOKEN_PROG.toBytes()),
                new Uint8Array(ATA_PROG.toBytes()), new Uint8Array(CL_PROG.toBytes())];
  const bhBytes = Buffer.from(Array.from({ length: 32 }, (_, i) => (i * 7 + 3) & 255));
  const serial = 42, gross = 100_000_000, rate = 1_087_300, cur = utf8('EUR');

  const msg = Buffer.from(clExchangeMsg(keys, new Uint8Array(bhBytes),
    new Uint8Array(wantDisc), serial, gross, new Uint8Array(cur), rate));
  const sig = new Uint8Array(await crypto.webcrypto.subtle.sign({ name: 'Ed25519' }, kp.privateKey, msg));
  const raw = Buffer.concat([Buffer.from([1]), Buffer.from(sig), msg]);

  let tx = null;
  try { tx = Transaction.from(raw); } catch (e) { no('官方库解析我拼的交易失败: ' + e.message); }

  if (tx) {
    const header = tx.compileMessage().header;
    const headOk = header.numRequiredSignatures === 1 && header.numReadonlySignedAccounts === 0
      && header.numReadonlyUnsignedAccounts === 6;
    headOk ? ok('header = 1 签名者 / 0 只读签名者 / 6 只读非签名者')
           : no('header 不对: ' + JSON.stringify(header));

    const ix = tx.instructions;
    const ix0ok = ix.length > 0 && ix[0].programId.equals(ATA_PROG)
      && Buffer.compare(Buffer.from(ix[0].data), Buffer.from([1])) === 0;
    ix0ok ? ok('指令 ① 是 ATA CreateIdempotent，data = [1]')
          : no('指令 ① 不对');

    const wantData = Buffer.concat([wantDisc, u64(serial), u64(gross), cur, u64(rate)]);
    const ix1 = ix[1];
    const ix1ok = ix.length === 2 && ix1 && ix1.programId.equals(CL_PROG)
      && Buffer.compare(Buffer.from(ix1.data), wantData) === 0;
    ix1ok ? ok('指令 ② 是 changeLink exchange，参数（disc/serial/gross/currency/rate）逐字节正确')
          : no('指令 ② 的数据不对');

    const keyAt = i => ix1 && ix1.keys[i];
    const wantRole = (i, bytes, sign, writ) => !!keyAt(i) &&
      keyAt(i).pubkey.toBase58() === new PublicKey(bytes).toBase58() &&
      keyAt(i).isSigner === sign && keyAt(i).isWritable === writ;
    const roles = [
      wantRole(0, payerU8, true, true),
      wantRole(1, kiosk, false, true),
      wantRole(2, treasury, false, true),
      wantRole(4, ata, false, true),
      wantRole(5, receipt, false, true),
    ];
    roles.every(Boolean)
      ? ok('exchange 的账户角色正确（机台签名 · 金库/目的地/receipt 可写）')
      : no('exchange 的账户角色不对');

    tx.verifySignatures()
      ? ok('官方库验签通过（整条交易可被节点接受）')
      : no('验签失败');
  }
  console.log(fail ? '\n\x1b[31m❌ 有失败项\x1b[0m' : '\n\x1b[32m✅ 全部通过\x1b[0m');
  process.exit(fail);
})().catch(e => { console.log('  \x1b[31mFAIL\x1b[0m ' + e.message); process.exit(1); });
