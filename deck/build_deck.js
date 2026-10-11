/* 零钱兑换机 · 逻辑链演示 deck（中文版 · 精简 7 页）
   构建：NODE_PATH=/opt/homebrew/lib/node_modules node build_deck.js
   字体：微软雅黑（构建后由 fix_cjk.py 给 a:latin 补 a:ea / a:cs）
   取舍：问题页保留完整证据；其余每页一句核心 + 一块带过的支撑；目录与章节分隔页省掉。
*/
const pptxgen = require("pptxgenjs");

const p = new pptxgen();
p.defineLayout({ name: "W16x9", width: 13.333, height: 7.5 });
p.layout = "W16x9";
p.title = "零钱兑换机 · 逻辑链演示";
p.author = "Xinyan Liu";

/* ---------- 调色板 ---------- */
const BG = "F7F7F5", WHITE = "FFFFFF", INK = "1F2937", MUTED = "6B7280";
const IND = "4F46E5", TEAL = "0D9488", CORAL = "E85D4A", DARK = "1E1B4B";
const TINDI = "EEEDF8", TINTT = "E6F4F2", TINTC = "FDECE9", LINE = "E3E3DE";
const F = "微软雅黑";

/* ---------- 字号表（唯一来源） ---------- */
const T = {
  cover: 40, coverSub: 18, tit: 26, h: 15.5, body: 14.5,
  small: 13.5, label: 12.5, foot: 12.5, focal: 40, num: 30
};
const TOTAL = 7;

/* ---------- 工具 ---------- */
const shadow = () => ({ type: "outer", color: "000000", opacity: 0.10, blur: 9, offset: 2, angle: 90 });

function slide(bg) {
  const s = p.addSlide();
  s.background = { color: bg || BG };
  return s;
}
function head(s, part, title, pageNo) {
  s.addText(part, { x: 0.75, y: 0.32, w: 7.5, h: 0.3, fontFace: F, fontSize: T.label, color: MUTED, margin: 0 });
  s.addText(`${pageNo} / ${TOTAL}`, { x: 10.6, y: 0.32, w: 1.98, h: 0.3, fontFace: F, fontSize: T.label, color: MUTED, align: "right", margin: 0 });
  s.addText(title, { x: 0.75, y: 0.68, w: 11.83, h: 1.0, fontFace: F, fontSize: T.tit, bold: true, color: INK, margin: 0, valign: "top" });
}
function foot(s, text) {
  s.addText(text, { x: 0.75, y: 6.93, w: 11.83, h: 0.32, fontFace: F, fontSize: T.foot, color: MUTED, margin: 0 });
}
function card(s, x, y, w, h, o) {
  o = o || {};
  s.addShape(p.ShapeType.roundRect, {
    x, y, w, h, rectRadius: 0.06,
    fill: { color: o.fill || WHITE },
    line: { color: o.line || LINE, width: o.lw || 1 },
    shadow: o.shadow === false ? undefined : shadow()
  });
}
function bar(s, x, y, w, h, fill, acc, label, text, lcol) {
  s.addShape(p.ShapeType.rect, { x, y, w, h, fill: { color: fill }, line: { color: fill, width: 0.5 } });
  s.addShape(p.ShapeType.rect, { x, y, w: 0.075, h, fill: { color: acc }, line: { color: acc, width: 0.5 } });
  let cy = y + 0.16;
  if (label) {
    s.addText(label, { x: x + 0.32, y: cy, w: w - 0.6, h: 0.3, fontFace: F, fontSize: T.label, bold: true, color: lcol || acc, margin: 0 });
    cy += 0.32;
  }
  s.addText(text, { x: x + 0.32, y: cy, w: w - 0.6, h: h - (cy - y) - 0.1, fontFace: F, fontSize: T.body, color: INK, margin: 0, lineSpacingMultiple: 1.15 });
}
function badge(s, x, y, d, txt, col) {
  s.addShape(p.ShapeType.ellipse, { x, y, w: d, h: d, fill: { color: col }, line: { color: col, width: 0.5 } });
  s.addText(txt, { x, y, w: d, h: d, fontFace: F, fontSize: 15, bold: true, color: WHITE, align: "center", valign: "middle", margin: 0 });
}

/* ============================================================
   1 封面
   ============================================================ */
{
  const s = slide(DARK);
  s.addShape(p.ShapeType.rect, { x: 0, y: 0, w: 0.22, h: 7.5, fill: { color: IND }, line: { color: IND, width: 0.5 } });
  s.addText("机场 / 景区 自助兑换设备", { x: 1.1, y: 1.6, w: 10, h: 0.4, fontFace: F, fontSize: T.h, color: "9BA0C8", margin: 0, charSpacing: 2 });
  s.addText("零钱兑换机", { x: 1.1, y: 2.25, w: 11.2, h: 1.5, fontFace: F, fontSize: T.cover, bold: true, color: "FFFFFF", margin: 0 });
  s.addText("把离境时花不掉的硬币，变成能带走的钱", { x: 1.1, y: 3.85, w: 11.2, h: 0.6, fontFace: F, fontSize: T.coverSub, color: "C9CCE8", margin: 0 });
  s.addShape(p.ShapeType.rect, { x: 1.12, y: 4.75, w: 3.2, h: 0.04, fill: { color: "4B4F7A" }, line: { color: "4B4F7A", width: 0.5 } });
  s.addText("逻辑链演示 · 2026 年 10 月 · 内部讨论稿", { x: 1.1, y: 5.05, w: 11.2, h: 0.4, fontFace: F, fontSize: T.small, color: "8A8FB0", margin: 0 });
}

/* ============================================================
   2 问题（核心页，保留完整证据）
   ============================================================ */
{
  const s = slide();
  head(s, "问题", "柜台不收硬币，机场汇率加价约 10%，零钱只能扔掉", 2);

  card(s, 0.75, 2.0, 6.05, 4.05, { line: CORAL, lw: 2.5 });
  s.addText("结构性的洞", { x: 1.1, y: 2.28, w: 4, h: 0.35, fontFace: F, fontSize: T.label, bold: true, color: CORAL, margin: 0, charSpacing: 1 });
  s.addText("“rarely buy or sell coins”", { x: 1.1, y: 2.68, w: 5.4, h: 1.0, fontFace: F, fontSize: 27, bold: true, color: INK, margin: 0 });
  s.addText("兑换商基本不收硬币：理由是硬币的仓储和运输成本高。整个行业只放弃了硬币这一块，纸币照收。",
    { x: 1.1, y: 3.78, w: 5.35, h: 1.0, fontFace: F, fontSize: T.body, color: INK, margin: 0, lineSpacingMultiple: 1.3 });
  s.addShape(p.ShapeType.rect, { x: 1.1, y: 4.9, w: 5.35, h: 0.02, fill: { color: CORAL }, line: { color: CORAL, width: 0.5 } });
  s.addText("硬币是唯一一块没人愿意收的零钱：这就是我们切进去的那条缝。",
    { x: 1.1, y: 5.1, w: 5.35, h: 0.8, fontFace: F, fontSize: T.h, bold: true, color: INK, margin: 0, lineSpacingMultiple: 1.25 });

  s.addText("机场柜台", { x: 7.15, y: 2.0, w: 5.4, h: 0.3, fontFace: F, fontSize: T.label, bold: true, color: MUTED, margin: 0, charSpacing: 1 });
  s.addShape(p.ShapeType.rect, { x: 7.15, y: 2.36, w: 5.43, h: 0.02, fill: { color: LINE }, line: { color: LINE, width: 0.5 } });
  s.addText("汇率加价约 10%（上下浮动），行业自己的建议是“只在紧急时换”。",
    { x: 7.15, y: 2.5, w: 5.43, h: 1.0, fontFace: F, fontSize: T.body, color: INK, margin: 0, lineSpacingMultiple: 1.3 });

  s.addText("曾经做过这件事的公司", { x: 7.15, y: 3.65, w: 5.4, h: 0.3, fontFace: F, fontSize: T.label, bold: true, color: MUTED, margin: 0, charSpacing: 1 });
  s.addShape(p.ShapeType.rect, { x: 7.15, y: 4.01, w: 5.43, h: 0.02, fill: { color: LINE }, line: { color: LINE, width: 0.5 } });
  s.addText("TravelersBox 自己收 3 至 10% 手续费，而且主轴已经退回礼卡和捐款：电子钱包那条路被它自己放弃了。",
    { x: 7.15, y: 4.15, w: 5.43, h: 1.3, fontFace: F, fontSize: T.body, color: INK, margin: 0, lineSpacingMultiple: 1.3 });

  s.addText("市场：只打硬币文化还在的地方", { x: 7.15, y: 5.5, w: 5.4, h: 0.3, fontFace: F, fontSize: T.label, bold: true, color: MUTED, margin: 0, charSpacing: 1 });
  s.addShape(p.ShapeType.rect, { x: 7.15, y: 5.86, w: 5.43, h: 0.02, fill: { color: LINE }, line: { color: LINE, width: 0.5 } });
  s.addText("日本 · 德国 · 土耳其 · 东南亚。现金整体在退潮，所以不赌大势。",
    { x: 7.15, y: 5.98, w: 5.43, h: 0.4, fontFace: F, fontSize: T.small, color: INK, margin: 0 });

  foot(s, "来源：Wikipedia “Bureau de change” · airportbanking.com · travelersbox.com（官网 FAQ）· Wikipedia “Cash”");
}

/* ============================================================
   3 解法 + 为什么是稳定币（简要）
   ============================================================ */
{
  const s = slide();
  head(s, "解法", "一台机器，四步，把硬币换成带得走的钱", 3);
  const steps = [
    ["1", "投钞", "验钞与清点"],
    ["2", "计价", "按链上汇率锁定金额"],
    ["3", "扫码", "读你的收款二维码"],
    ["4", "到账", "稳定币进你自己的钱包"]
  ];
  const w = 2.7, gap = 0.34;
  steps.forEach((st, i) => {
    const x = 0.75 + i * (w + gap);
    card(s, x, 2.0, w, 1.95, {});
    badge(s, x + 0.28, 2.25, 0.5, st[0], i === 3 ? TEAL : IND);
    s.addText(st[1], { x: x + 0.92, y: 2.3, w: w - 1.2, h: 0.42, fontFace: F, fontSize: T.h + 1, bold: true, color: INK, margin: 0, valign: "middle" });
    s.addText(st[2], { x: x + 0.28, y: 2.95, w: w - 0.56, h: 0.8, fontFace: F, fontSize: T.body, color: MUTED, margin: 0, lineSpacingMultiple: 1.25 });
    if (i < 3) s.addText("▶", { x: x + w - 0.02, y: 2.8, w: gap + 0.04, h: 0.4, fontFace: F, fontSize: 13, color: "C7C7C0", align: "center", margin: 0 });
  });
  bar(s, 0.75, 4.15, 11.83, 1.05, TINTT, TEAL, "为什么是稳定币",
    "能带回国、24 小时到账、不依赖当地银行。机器本身不是难点：验钞和二维码摄像头都是现成件。", TEAL);
  bar(s, 0.75, 5.35, 11.83, 1.35, TINTC, CORAL, "但先认账一件事",
    "稳定币今天的真实用户是跨境汇款和新兴市场的人，不是游客手里那 3 美元零钱。所以第一步的客户不是游客，游客只是入口场景。", CORAL);
}

/* ============================================================
   4 合规（简要）
   ============================================================ */
{
  const s = slide();
  head(s, "能不能活", "现金换成加密，是反洗钱监管最盯的组合", 4);
  const rows = [
    ["A", "全 KYC", "合法，但过境旅客做不完身份核验，体验直接死。", false],
    ["B", "分层 KYC", "小额免身份、大额要身份。体验和合规各保一半。", true],
    ["C", "外包合规", "你只做入口，牌照和 KYC 交给持牌机构。", true]
  ];
  let y = 2.0;
  rows.forEach((r) => {
    card(s, 0.75, y, 11.83, 1.0, { line: r[3] ? IND : LINE, lw: r[3] ? 2.5 : 1 });
    badge(s, 1.05, y + 0.25, 0.5, r[0], r[3] ? IND : "9CA3AF");
    s.addText(r[1], { x: 1.75, y: y + 0.28, w: 2.4, h: 0.45, fontFace: F, fontSize: T.h, bold: true, color: INK, margin: 0, valign: "middle" });
    s.addText(r[2], { x: 4.3, y: y + 0.28, w: 6.6, h: 0.45, fontFace: F, fontSize: T.body, color: INK, margin: 0, valign: "middle" });
    if (r[3]) s.addText("我们选这个", { x: 11.0, y: y + 0.3, w: 1.4, h: 0.4, fontFace: F, fontSize: T.label, bold: true, color: IND, align: "right", margin: 0 });
    y += 1.15;
  });
  bar(s, 0.75, 5.35, 11.83, 1.35, TINDI, IND, "结论",
    "合规不是负担，是入场券：机场和银行只跟有牌照的机构合作。", IND);
  foot(s, "来源：FATF《Targeted Report on Stablecoins and Unhosted Wallets》（2024）");
}

/* ============================================================
   5 算账（简要）
   ============================================================ */
{
  const s = slide();
  head(s, "能不能活", "手续费撑不起线下生意", 5);

  card(s, 0.75, 2.05, 5.85, 4.05, {});
  s.addText("一次兑换的账（我们的推算）", { x: 1.1, y: 2.32, w: 5.1, h: 0.35, fontFace: F, fontSize: T.label, bold: true, color: MUTED, margin: 0, charSpacing: 1 });
  s.addText("0.15 美元", { x: 1.1, y: 2.75, w: 5.1, h: 0.95, fontFace: F, fontSize: T.focal, bold: true, color: CORAL, margin: 0 });
  s.addText("一次剩 3 美元零钱 × 5% 手续费", { x: 1.1, y: 3.68, w: 5.1, h: 0.4, fontFace: F, fontSize: T.body, color: INK, margin: 0 });
  s.addShape(p.ShapeType.rect, { x: 1.1, y: 4.15, w: 5.1, h: 0.02, fill: { color: LINE }, line: { color: LINE, width: 0.5 } });
  s.addText("而固定成本是：机场租金、验钞硬件、维保、押运、客服。",
    { x: 1.1, y: 4.35, w: 5.1, h: 0.8, fontFace: F, fontSize: T.body, color: INK, margin: 0, lineSpacingMultiple: 1.25 });
  s.addText("按公开费率估算，不是对方公布的数据。",
    { x: 1.1, y: 5.3, w: 5.1, h: 0.4, fontFace: F, fontSize: T.small, color: MUTED, margin: 0 });

  s.addText("结论", { x: 7.15, y: 2.05, w: 5.4, h: 0.3, fontFace: F, fontSize: T.label, bold: true, color: IND, margin: 0, charSpacing: 1 });
  s.addShape(p.ShapeType.rect, { x: 7.15, y: 2.41, w: 5.43, h: 0.02, fill: { color: IND }, line: { color: IND, width: 0.5 } });
  s.addText("单靠手续费，单机模型不成立。",
    { x: 7.15, y: 2.6, w: 5.43, h: 0.55, fontFace: F, fontSize: 19, bold: true, color: INK, margin: 0 });

  s.addText("钱在哪", { x: 7.15, y: 3.45, w: 5.4, h: 0.3, fontFace: F, fontSize: T.label, bold: true, color: IND, margin: 0, charSpacing: 1 });
  s.addShape(p.ShapeType.rect, { x: 7.15, y: 3.81, w: 5.43, h: 0.02, fill: { color: LINE }, line: { color: LINE, width: 0.5 } });
  s.addText("把零钱导给品牌方：礼卡、兑换佣金、导流费。旅客不额外付钱，掏钱的是品牌方。",
    { x: 7.15, y: 3.98, w: 5.43, h: 1.1, fontFace: F, fontSize: T.body, color: INK, margin: 0, lineSpacingMultiple: 1.3 });

  s.addText("门槛在哪", { x: 7.15, y: 5.15, w: 5.4, h: 0.3, fontFace: F, fontSize: T.label, bold: true, color: IND, margin: 0, charSpacing: 1 });
  s.addShape(p.ShapeType.rect, { x: 7.15, y: 5.51, w: 5.43, h: 0.02, fill: { color: LINE }, line: { color: LINE, width: 0.5 } });
  s.addText("不在技术，在牌照和机场场地：技术谁都能抄。",
    { x: 7.15, y: 5.65, w: 5.43, h: 0.5, fontFace: F, fontSize: T.body, color: INK, margin: 0 });

  foot(s, "来源：travelersbox.com（费率）· AirportBanking（机场成本结构）");
}

/* ============================================================
   6 三件事决定生死（简要）
   ============================================================ */
{
  const s = slide();
  head(s, "能不能活", "这三件事决定项目生死", 6);
  const rows = [
    ["01", "痛点强度", "多少人愿意专门处理剩下的零钱？", "目标市场做访谈，看转化率"],
    ["02", "为什么非稳定币", "现在只有“能带回国”这一条理由。", "把客户从游客换成跨境用户，重新论证"],
    ["03", "单机模型", "租金加硬件，账还没算过。", "拿一个真机场的报价单来算"]
  ];
  let y = 2.0;
  rows.forEach((r) => {
    card(s, 0.75, y, 11.83, 1.35, {});
    s.addText(r[0], { x: 1.05, y: y + 0.32, w: 1.0, h: 0.6, fontFace: F, fontSize: T.num, bold: true, color: CORAL, margin: 0 });
    s.addText(r[1], { x: 2.2, y: y + 0.2, w: 3.6, h: 0.4, fontFace: F, fontSize: T.h, bold: true, color: INK, margin: 0 });
    s.addText(r[2], { x: 2.2, y: y + 0.66, w: 9.9, h: 0.45, fontFace: F, fontSize: T.body, color: MUTED, margin: 0 });
    s.addText("→ " + r[3], { x: 6.0, y: y + 0.66, w: 6.3, h: 0.45, fontFace: F, fontSize: T.body, bold: true, color: INK, margin: 0 });
    y += 1.55;
  });
  s.addText("这三条没有结论之前，不做硬件定型，也不谈机场。", { x: 0.75, y: 6.7, w: 11.83, h: 0.4, fontFace: F, fontSize: T.small, bold: true, color: IND, margin: 0 });
}

/* ============================================================
   7 收尾
   ============================================================ */
{
  const s = slide(DARK);
  s.addShape(p.ShapeType.rect, { x: 0, y: 0, w: 0.22, h: 7.5, fill: { color: IND }, line: { color: IND, width: 0.5 } });
  s.addText("把口袋里的硬币，做成一条跨境价值通道的入口", { x: 1.1, y: 2.4, w: 11.2, h: 1.7, fontFace: F, fontSize: 32, bold: true, color: "FFFFFF", margin: 0, lineSpacingMultiple: 1.25 });
  s.addShape(p.ShapeType.roundRect, { x: 1.1, y: 4.4, w: 8.6, h: 0.95, rectRadius: 0.06, fill: { color: "2A2A5E" }, line: { color: "4F46E5", width: 1.5 } });
  s.addText("我们现在要的：一个机场的试点位，和一条合规路径的确认。", { x: 1.4, y: 4.4, w: 8.1, h: 0.95, fontFace: F, fontSize: T.h, color: "E4E4F7", valign: "middle", margin: 0 });
  s.addText("零钱兑换机 · 2026 年 10 月", { x: 1.1, y: 5.75, w: 11.2, h: 0.4, fontFace: F, fontSize: T.small, color: "8A8FB0", margin: 0 });
}

p.writeFile({ fileName: "零钱兑换机_逻辑链.pptx" }).then((f) => console.log("built ->", f));
