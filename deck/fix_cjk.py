# -*- coding: utf-8 -*-
"""给 pptxgenjs 生成的 .pptx 补 CJK 字体：<a:latin> 后面插 <a:ea> / <a:cs>。
否则 PowerPoint 会用主题的东亚字体渲染中文，微软雅黑只在西文上生效。
用法: /usr/bin/python3 fix_cjk.py 零钱兑换机_逻辑链.pptx
"""
import re, shutil, sys, zipfile

path = sys.argv[1]
tmp = path + ".tmp"
pat = re.compile(r'<a:latin typeface="([^"]+)"/>')

with zipfile.ZipFile(path) as zin:
    names = zin.namelist()
    data = {n: zin.read(n) for n in names}

hits = 0
for n in list(data):
    if re.match(r"ppt/(slides|slideLayouts|slideMasters|notesSlides)/.*\.xml$", n) or n == "ppt/theme/theme1.xml":
        s = data[n].decode("utf-8")
        def rep(m):
            global hits
            f = m.group(1)
            hits += 1
            return '<a:latin typeface="%s"/><a:ea typeface="%s"/><a:cs typeface="%s"/>' % (f, f, f)
        s2 = pat.sub(rep, s)
        data[n] = s2.encode("utf-8")

with zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED) as zout:
    for n in names:
        zout.writestr(n, data[n])
shutil.move(tmp, path)
print("已给 %d 处 typeface 补上 a:ea / a:cs" % hits)
