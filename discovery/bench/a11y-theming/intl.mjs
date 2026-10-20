// Prints `label<TAB>text` lines: what Intl formats for tick labels in a few locales,
// and labels with wide, combining and joined glyphs. Throwaway (see README.md).
// Run: mise exec -- node discovery/bench/a11y-theming/intl.mjs | target/release/widths
const n = -1234567.891;
const lines = [];
const num = (loc, opts = {}) => lines.push([`number ${loc} ${JSON.stringify(opts)}`, new Intl.NumberFormat(loc, opts).format(n)]);
for (const loc of ["en-US", "de-DE", "de-CH", "fr-FR", "en-IN", "ar-EG", "ar", "fa-IR", "he-IL", "hi-IN-u-nu-deva", "th-TH-u-nu-thai", "ja-JP", "zh-CN"]) num(loc);
num("ar-EG", { style: "percent" });
num("en-US", { notation: "compact" });
num("ja-JP", { notation: "compact" });
num("de-DE", { style: "currency", currency: "EUR" });
num("ar-EG", { style: "currency", currency: "EGP" });
const d = new Date(Date.UTC(2026, 9, 18, 14, 5, 9));
for (const loc of ["en-US", "en-GB", "de-DE", "ja-JP", "zh-CN", "ar-EG", "he-IL", "fa-IR", "ko-KR"]) {
  lines.push([`date ${loc}`, new Intl.DateTimeFormat(loc, { timeZone: "UTC", dateStyle: "medium", timeStyle: "short" }).format(d)]);
  lines.push([`month ${loc}`, new Intl.DateTimeFormat(loc, { timeZone: "UTC", month: "short" }).format(d)]);
}
lines.push(["label precomposed", "Café"]);
lines.push(["label decomposed", "Café"]);
lines.push(["label CJK", "東京証券取引所"]);
lines.push(["label Hangul", "서울"]);
lines.push(["label fullwidth digits", "１２３"]);
lines.push(["label emoji", "\u{1F44D}"]);
lines.push(["label emoji ZWJ family", "\u{1F468}‍\u{1F469}‍\u{1F467}"]);
lines.push(["label flag", "\u{1F1EF}\u{1F1F5}"]);
lines.push(["label emoji text-style", "☀"]);
lines.push(["label emoji VS16", "☀️"]);
lines.push(["label Hebrew", "מחיר"]);
lines.push(["label Arabic", "السعر"]);
lines.push(["label Devanagari", "हिन्दी"]);
lines.push(["label Thai", "ราคา"]);
lines.push(["label Arabic-Indic digits", "٠١٢٣٤٥٦٧٨٩"]);
lines.push(["label Extended Arabic-Indic digits", "۰۱۲۳۴۵۶۷۸۹"]);
for (const [l, t] of lines) console.log(`${l}\t${t}`);
