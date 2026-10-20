// The locale: the element's (REQ-I18N-010), and the locale pack built from
// Intl with the Gregorian calendar forced, once per locale
// (REQ-I18N-020, spec/schemas/locale.schema.json). The browser's minus is
// U+2212 (REQ-FMT-060). No wasm module links a locale database; the pack
// crosses as JSON through `set_locale`.

/** A locale pack (locale.schema.json). */
export interface LocalePack {
  decimal: string;
  group: string;
  grouping: number[];
  minus: string;
  percent: string;
  numerals: string[];
  currency: [string, string];
  months: string[];
  monthsShort: string[];
  days: string[];
  daysShort: string[];
  periods: [string, string];
  dateTime: string;
  date: string;
  time: string;
  hourCycle: "h12" | "h23";
  direction: "ltr" | "rtl";
}

/** The minus sign the browser draws (REQ-FMT-060). */
export const BROWSER_MINUS = "−";

/**
 * The element's locale (REQ-I18N-010): its `locale` attribute, else the
 * `lang` of its nearest ancestor that has one, else `navigator.language`,
 * else `en-US`.
 */
export function localeOf(el: Element | null): string {
  const own = el?.getAttribute("locale");
  if (own) return own;
  const withLang = el?.closest("[lang]");
  const lang = withLang?.getAttribute("lang");
  if (lang) return lang;
  const nav = (globalThis as { navigator?: { language?: string } }).navigator;
  return nav?.language || "en-US";
}

/** Common regions' currencies, for the pack's currency affixes; anything else takes USD. The page picks the currency itself. */
const REGION_CURRENCY: Record<string, string> = {
  US: "USD", GB: "GBP", JP: "JPY", CN: "CNY", IN: "INR", CH: "CHF", CA: "CAD", AU: "AUD", KR: "KRW", BR: "BRL",
  RU: "RUB", MX: "MXN", SE: "SEK", NO: "NOK", DK: "DKK", PL: "PLN", CZ: "CZK", HU: "HUF", TR: "TRY", IL: "ILS",
  EG: "EGP", SA: "SAR", AE: "AED", IR: "IRR", TH: "THB", ZA: "ZAR", NZ: "NZD", SG: "SGD", HK: "HKD", TW: "TWD",
  DE: "EUR", FR: "EUR", IT: "EUR", ES: "EUR", NL: "EUR", BE: "EUR", AT: "EUR", PT: "EUR", FI: "EUR", IE: "EUR", GR: "EUR",
};

/** A tag with the Gregorian calendar forced, and its resolved form; an invalid tag falls back to en-US. */
export function gregorian(tag: string): string {
  try {
    return new Intl.Locale(tag, { calendar: "gregory" }).toString();
  } catch {
    return new Intl.Locale("en-US", { calendar: "gregory" }).toString();
  }
}

/** Group sizes from the decimal point leftward, the last repeating, as d3-format's `grouping`. */
export function groupingOf(integerParts: readonly string[]): number[] {
  const sizes = integerParts.map((s) => s.length).reverse();
  if (sizes.length <= 1) return [3];
  // The leftmost group may be short: drop it.
  const full = sizes.slice(0, -1);
  const out: number[] = [];
  for (const s of full) if (out.length === 0 || s !== out[out.length - 1]) out.push(s);
  return out.length === 0 ? [3] : out;
}

/** A strftime pattern from DateTimeFormat parts (the subset of REQ-FMT-050). */
function pattern(parts: readonly Intl.DateTimeFormatPart[], h12: boolean, opts: { day?: "%d" | "%e"; month?: "%m" | "%b"; weekday?: "%a" }): string {
  return parts
    .map((p) => {
      switch (p.type) {
        case "year":
          return "%Y";
        case "month":
          // A month written as digits (ja: 1月) takes %m, not a name.
          return /^\p{Nd}+$/u.test(p.value) ? "%m" : (opts.month ?? "%m");
        case "day":
          return opts.day ?? "%d";
        case "weekday":
          return opts.weekday ?? "%a";
        case "hour":
          return h12 ? "%I" : "%H";
        case "minute":
          return "%M";
        case "second":
          return "%S";
        case "dayPeriod":
          return "%p";
        case "literal":
          return p.value.replace(/%/g, "%%");
        default:
          return "";
      }
    })
    .join("");
}

const packs = new Map<string, LocalePack>();

/**
 * The locale pack for `tag`, built from Intl with the Gregorian calendar
 * forced, once per tag (REQ-I18N-020). The minus is U+2212 unless `minus`
 * says otherwise (REQ-FMT-060); `currency` picks the affixes' currency.
 * Text is NFC-normalised (REQ-I18N-050).
 */
export function localePack(tag: string, opts: { minus?: string; currency?: string } = {}): LocalePack {
  const key = `${tag}\u0000${opts.minus ?? ""}\u0000${opts.currency ?? ""}`;
  const known = packs.get(key);
  if (known) return known;
  const loc = gregorian(tag);
  const nf = new Intl.NumberFormat(loc);
  const parts = nf.formatToParts(-1234567890123.5);
  const one = (t: string): string => parts.find((p) => p.type === t)?.value ?? "";
  const integers = parts.filter((p) => p.type === "integer").map((p) => p.value);
  const plain = new Intl.NumberFormat(loc, { useGrouping: false });
  const numerals = Array.from({ length: 10 }, (_, d) => plain.format(d));
  const percent = new Intl.NumberFormat(loc, { style: "percent" })
    .formatToParts(0.5)
    .map((p) => (p.type === "integer" ? "#" : p.value))
    .join("");
  const region = (() => {
    try {
      return new Intl.Locale(loc).maximize().region ?? "";
    } catch {
      return "";
    }
  })();
  const currency = opts.currency ?? REGION_CURRENCY[region] ?? "USD";
  const cparts = new Intl.NumberFormat(loc, { style: "currency", currency }).formatToParts(1);
  const firstNum = cparts.findIndex((p) => p.type === "integer");
  let lastNum = -1;
  cparts.forEach((p, i) => {
    if (p.type === "integer" || p.type === "fraction" || p.type === "decimal" || p.type === "group") lastNum = i;
  });
  const affix = (ps: Intl.NumberFormatPart[]): string => ps.map((p) => p.value).join("");
  const dtf = (o: Intl.DateTimeFormatOptions): Intl.DateTimeFormat => new Intl.DateTimeFormat(loc, { timeZone: "UTC", ...o });
  const months = (month: "long" | "short"): string[] => Array.from({ length: 12 }, (_, m) => dtf({ month }).format(Date.UTC(2001, m, 15)));
  // 1970-01-04 was a Sunday.
  const days = (weekday: "long" | "short"): string[] => Array.from({ length: 7 }, (_, d) => dtf({ weekday }).format(Date.UTC(1970, 0, 4 + d)));
  const hc = dtf({ hour: "numeric" }).resolvedOptions().hourCycle;
  const h12 = hc === "h11" || hc === "h12";
  const period = (h: number): string => dtf({ hour: "numeric", hour12: true }).formatToParts(Date.UTC(2001, 0, 1, h)).find((p) => p.type === "dayPeriod")?.value ?? (h < 12 ? "AM" : "PM");
  const sample = Date.UTC(2001, 0, 2, 15, 4, 5);
  const date = pattern(dtf({ year: "numeric", month: "2-digit", day: "2-digit" }).formatToParts(sample), h12, {});
  const time = pattern(dtf({ hour: "2-digit", minute: "2-digit", second: "2-digit" }).formatToParts(sample), h12, {});
  const dateTime = pattern(
    dtf({ weekday: "short", year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", second: "2-digit" }).formatToParts(sample),
    h12,
    { day: "%e", month: "%b", weekday: "%a" },
  );
  let direction: "ltr" | "rtl" = "ltr";
  try {
    const l = new Intl.Locale(loc) as Intl.Locale & { getTextInfo?: () => { direction?: string }; textInfo?: { direction?: string } };
    const info = l.getTextInfo?.() ?? l.textInfo;
    if (info?.direction === "rtl") direction = "rtl";
  } catch {
    // ltr
  }
  const n = (s: string): string => s.normalize("NFC");
  const pack: LocalePack = {
    decimal: n(one("decimal") || "."),
    group: n(one("group")),
    grouping: groupingOf(integers),
    minus: opts.minus ?? BROWSER_MINUS,
    percent: n(percent.includes("#") ? percent : "#%"),
    numerals: numerals.map(n),
    currency: [n(affix(cparts.slice(0, Math.max(0, firstNum)))), n(affix(cparts.slice(lastNum + 1)))],
    months: months("long").map(n),
    monthsShort: months("short").map(n),
    days: days("long").map(n),
    daysShort: days("short").map(n),
    periods: [n(period(1)), n(period(13))],
    dateTime: n(dateTime),
    date: n(date),
    time: n(time),
    hourCycle: h12 ? "h12" : "h23",
    direction,
  };
  packs.set(key, pack);
  return pack;
}
