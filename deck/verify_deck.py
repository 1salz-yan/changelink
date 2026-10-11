# -*- coding: utf-8 -*-
"""交付前自检：页数 / 破折号 / 最小字号 / 每页文字转储。
用法: /usr/bin/python3 verify_deck.py 零钱兑换机_逻辑链.pptx
"""
import re, sys, zipfile

path = sys.argv[1]
z = zipfile.ZipFile(path)
assert z.testzip() is None, "zip 损坏"

# 幻灯片顺序按 sldIdLst
pres = z.read("ppt/presentation.xml").decode("utf-8")
rels = z.read("ppt/_rels/presentation.xml.rels").decode("utf-8")
rid2t = dict(re.findall(r'Id="(rId\d+)"[^>]*Target="([^"]+)"', rels))
order = [rid2t[r] for r in re.findall(r'<p:sldId[^>]*r:id="(rId\d+)"', pres)]
print("页数:", len(order))

total_chars, sizes = 0, []
for i, tgt in enumerate(order, 1):
    name = "ppt/" + tgt.replace("../", "")
    xml = z.read(name).decode("utf-8")
    texts = re.findall(r"<a:t>([^<]*)</a:t>", xml)
    joined = " | ".join(t for t in texts if t.strip())
    total_chars += len(joined)
    for m in re.finditer(r'<a:rPr[^>]*sz="(\d+)"', xml):
        sizes.append(int(m.group(1)) / 100.0)
    print("\n--- 第 %d 页 (%d 字) ---" % (i, len(joined)))
    print(joined)

bad = re.findall(r"[—–]", " ".join(re.findall(r"<a:t>([^<]*)</a:t>", " ".join(
    z.read("ppt/" + t.replace("../", "")).decode("utf-8") for t in order))))
print("\n破折号(—/–)出现次数:", len(bad))
print("正文字号分布:", sorted(set(sizes)))
small = sorted(set(s for s in sizes if s < 12))
print("低于 12pt 的字号:", small if small else "无")
print("总字数:", total_chars)
