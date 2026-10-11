#!/bin/bash
# 一键重建 + 渲染 PDF。字体变量必须显式给 soffice，否则它静默换字体。
set -e
cd "$(dirname "$0")"
export FONTCONFIG_FILE="$PWD/fonts.conf"
export FONTCONFIG_PATH="$PWD"
NODE_PATH=/opt/homebrew/lib/node_modules node build_deck.js
/usr/bin/python3 fix_cjk.py 零钱兑换机_逻辑链.pptx
/usr/bin/python3 verify_deck.py 零钱兑换机_逻辑链.pptx | head -5
soffice --headless --convert-to pdf --outdir . 零钱兑换机_逻辑链.pptx
ls -l 零钱兑换机_逻辑链.pptx 零钱兑换机_逻辑链.pdf
