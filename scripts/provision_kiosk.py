#!/usr/bin/env python3
"""给 ChangeLink 的浏览器演示准备一台机台。

做三件事，都不需要机台那把私钥：
  1. 建一枚演示稳定币（铸币权在操作员这把命令行钱包手里）
  2. 建机台金库 —— 一个 owner 是**机台 PDA** 的 token account
  3. 往金库里放演示稳定币

机台自己的身份（机台的 config 账户）不在这里注册：那一步由机台自己的钱包签名，
在浏览器里打开 atm.html#provision 完成。机台的身份属于机器，不属于操作员的电脑。

用法
----
    ~/.venvs/letterbox/bin/python scripts/provision_kiosk.py --wallet <浏览器里那台机台的钱包地址>

钱包地址从哪来：打开 atm.html，右侧卡片上的 kiosk wallet，点 "Copy kiosk address"。
"""

import argparse
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import devnet_smoke as s  # 同一套字节布局、同一套 RPC 重试 —— 不写第二遍


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--wallet", required=True,
                    help="机台的链上 authority = 浏览器里那把机台钱包的地址")
    ap.add_argument("--kiosk-id", type=int, default=20261017,
                    help="机器序列号（PDA 种子）；同一台机器重跑要用同一个号")
    ap.add_argument("--fund", type=float, default=1000.0,
                    help="往金库放的演示稳定币数量")
    ap.add_argument("--rpc", default="http://127.0.0.1:8899")
    args = ap.parse_args()

    # 地址必须真的 base58 解出 32 字节 —— 只查形状会放非法地址过关
    wallet = s.Pubkey.from_string(args.wallet)
    payer = s.Keypair.from_json(
        pathlib.Path("~/.config/solana/id.json").expanduser().read_text()
    )
    chain = s.Chain(args.rpc)

    print(f"RPC       {args.rpc}")
    print(f"operator  {payer.pubkey()}   (建币、建金库、放钱)")
    print(f"kiosk     {wallet}   (机台自己的钱包，签名注册自己)\n")

    # 1. 演示稳定币
    mint = s.Keypair()
    chain.create_data_account(payer, mint, s.MINT_LEN, s.TOKEN_PROGRAM, "create mint account")
    chain.send([s.ix_initialize_mint2(mint.pubkey(), s.DECIMALS, payer.pubkey())],
               payer, label="initialize mint")

    # 2. 金库：owner 必须是机台 PDA，程序的 initialize_kiosk 会检查这一条
    #    先挑一个链上还空着的序列号 —— 同一个序列号的 PDA 已经存在就没法再注册了
    kiosk_id, kiosk, bump = args.kiosk_id, None, 0
    for offset in range(200):
        kiosk_id = args.kiosk_id + offset
        addr, b = s.Pubkey.find_program_address(
            [s.KIOSK_SEED, kiosk_id.to_bytes(8, "little")], s.PROGRAM_ID
        )
        if chain.account(addr) is None:
            kiosk, bump = addr, b
            break
    if kiosk is None:
        raise SystemExit("could not find a free kiosk serial")
    if kiosk_id != args.kiosk_id:
        print(f"  (serial {args.kiosk_id} is taken on-chain → using {kiosk_id})\n")

    treasury = s.Keypair()
    chain.create_data_account(payer, treasury, s.TOKEN_ACCOUNT_LEN, s.TOKEN_PROGRAM,
                              "create treasury account")
    chain.send([s.ix_initialize_account3(treasury.pubkey(), mint.pubkey(), kiosk)],
               payer, label="initialize treasury")

    # 3. 放钱
    amount = int(round(args.fund * s.ONE))
    chain.send([s.ix_mint_to(mint.pubkey(), treasury.pubkey(), payer.pubkey(), amount)],
               payer, label=f"fund treasury ({args.fund:g})")

    print(f"\n  kiosk PDA  {kiosk}  (bump {bump})\n")

    print("把这四个值贴进 atm.html 里 var CL = { … }：\n")
    print(f"  kioskId:{kiosk_id},")
    print(f"  kiosk:'{kiosk}',")
    print(f"  mint:'{mint.pubkey()}',")
    print(f"  treasury:'{treasury.pubkey()}',\n")
    print("然后在浏览器里打开一次（机台自己的钱包签名注册这台机器）：\n")
    print("  ⤷ atm.html#provision\n")
    print("机台钱包还要有点 devnet SOL 付手续费和金库以外的租金；不够时界面会提示。")
    print(f"\n  程序  https://explorer.solana.com/address/{s.PROGRAM_ID}?cluster=devnet")
    return 0


if __name__ == "__main__":
    sys.exit(main())
