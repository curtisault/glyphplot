// locale.ts: the locale pack from Intl, Gregorian forced (REQ-I18N-020),
// U+2212 minus (REQ-FMT-060), and the element's locale (REQ-I18N-010).
import { test } from "node:test";
import assert from "node:assert/strict";
import { localePack, localeOf, groupingOf, gregorian, BROWSER_MINUS } from "../src/locale.ts";

const REQUIRED = ["decimal", "group", "grouping", "minus", "percent", "numerals", "currency", "months", "monthsShort", "days", "daysShort", "periods", "dateTime", "date", "time", "hourCycle", "direction"];

test("every field locale.schema.json requires, and no other", () => {
  for (const tag of ["en-US", "de-DE", "ar-EG", "ja-JP"]) {
    const p = localePack(tag);
    assert.deepEqual(Object.keys(p).sort(), [...REQUIRED].sort(), tag);
    assert.equal(p.numerals.length, 10);
    assert.equal(p.months.length, 12);
    assert.equal(p.days.length, 7);
    assert.ok(p.percent.includes("#"));
    assert.ok(p.grouping.every((g) => Number.isInteger(g) && g >= 1));
    assert.ok(["h12", "h23"].includes(p.hourCycle));
    for (const k of ["dateTime", "date", "time"]) assert.ok(p[k].length > 0);
  }
});

test("en-US and de-DE symbols, names and patterns", () => {
  const en = localePack("en-US");
  assert.deepEqual([en.decimal, en.group, en.grouping, en.percent, en.currency, en.hourCycle, en.direction], [".", ",", [3], "#%", ["$", ""], "h12", "ltr"]);
  assert.equal(en.days[0], "Sunday");
  assert.equal(en.months[0], "January");
  assert.equal(en.date, "%m/%d/%Y");
  assert.equal(en.time, "%I:%M:%S %p");
  const de = localePack("de-DE");
  assert.deepEqual([de.decimal, de.group, de.grouping, de.hourCycle], [",", ".", [3], "h23"]);
  assert.equal(de.percent, "# %");
  assert.deepEqual(de.currency, ["", " €"]);
  assert.equal(de.date, "%d.%m.%Y");
  assert.equal(de.months[2], "März");
});

test("the browser's minus is U+2212, switchable", () => {
  assert.equal(localePack("en-US").minus, "−");
  assert.equal(BROWSER_MINUS, "−");
  assert.equal(localePack("en-US", { minus: "-" }).minus, "-");
});

test("Indian grouping, Arabic digits, right-to-left", () => {
  assert.deepEqual(localePack("en-IN").grouping, [3, 2]);
  assert.deepEqual(groupingOf(["12", "34", "56", "78", "90", "123"]), [3, 2]);
  assert.deepEqual(groupingOf(["1"]), [3]);
  const ar = localePack("ar-EG");
  assert.equal(ar.numerals.join(""), "٠١٢٣٤٥٦٧٨٩");
  assert.equal(ar.direction, "rtl");
});

test("the Gregorian calendar is forced", () => {
  assert.match(gregorian("th-TH"), /-u-ca-gregory/);
  assert.match(gregorian("ja-JP-u-ca-japanese"), /ca-gregory/);
  assert.equal(localePack("fa-IR").months[0], "ژانویه", "January, not Farvardin");
  assert.equal(localePack("th-TH").months[0], "มกราคม");
});

test("one pack a locale", () => {
  assert.equal(localePack("fr-FR"), localePack("fr-FR"));
});

test("the element's locale: its attribute, the nearest lang, navigator.language", () => {
  const el = (attrs, parent = null) => ({
    getAttribute: (n) => attrs[n] ?? null,
    closest: (sel) => (sel === "[lang]" ? (attrs.lang ? el(attrs, parent) : parent?.closest(sel) ?? null) : null),
  });
  const page = el({ lang: "fr-CA" });
  assert.equal(localeOf(el({ locale: "de-DE" }, page)), "de-DE");
  assert.equal(localeOf(el({}, page)), "fr-CA");
  assert.equal(localeOf(el({}, null)), globalThis.navigator?.language || "en-US");
});
