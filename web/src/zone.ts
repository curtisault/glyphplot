// Time zones from the shell (REQ-TZ-020, REQ-TZ-050, ADR-112): the element's
// `timeZone` (`UTC`, a fixed offset or an IANA name; the page's own zone by
// default, resolved to its IANA name and passed explicitly), and the table of
// offset transitions the core draws local time by
// (spec/schemas/zone.schema.json), computed with Intl.DateTimeFormat over a
// span. The core reads no zone or clock (REQ-TZ-010); a view panned past the
// span a table covers asks for a new one.

/** From `at` on, local time is UTC plus `offset` minutes. */
export interface Transition {
  at: number;
  offset: number;
}

/** A zone table (zone.schema.json). */
export interface ZoneTable {
  name: string;
  transitions: Transition[];
  covering: [number, number];
}

const DAY = 86_400_000;
const SECOND = 1000;

/** How far apart the table's samples are: a zone changing offset twice within a day is missed (no zone in tzdata 2025 does since 1970). */
export const STEP_MS = DAY;

/** The page's zone, as Intl resolves it (REQ-TZ-050); UTC when Intl gives none. */
export function pageZone(): string {
  try {
    return new Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
}

/** The minutes of a fixed offset, `+05:30`, `-0800` or `+05`; null for anything else or past ±18 hours. */
export function fixedOffset(name: string): number | null {
  const m = /^([+-])(\d{2})(?::?(\d{2}))?$/.exec(name.trim());
  if (!m) return null;
  const h = Number(m[2]);
  const min = Number(m[3] ?? 0);
  if (min > 59) return null;
  const v = (h * 60 + min) * (m[1] === "-" ? -1 : 1);
  return Math.abs(v) <= 1080 ? v : null;
}

const formats = new Map<string, Intl.DateTimeFormat>();

/** A formatter giving `zone`'s wall-clock fields, made once a zone; throws a RangeError for a zone Intl does not know. */
function formatter(zone: string): Intl.DateTimeFormat {
  let f = formats.get(zone);
  if (!f) {
    f = new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      hourCycle: "h23",
      era: "short",
      year: "numeric",
      month: "numeric",
      day: "numeric",
      hour: "numeric",
      minute: "numeric",
      second: "numeric",
    });
    formats.set(zone, f);
  }
  return f;
}

/**
 * The zone a `timeZone` attribute names: `UTC` for UTC (or GMT, Z), a fixed
 * offset as written, an IANA name as written; the page's zone when it is
 * empty or absent (REQ-TZ-050). Null for a name Intl does not know.
 */
export function resolveZone(name: string | null | undefined): string | null {
  const s = (name ?? "").trim();
  if (s === "") return pageZone();
  if (/^(utc|gmt|z)$/i.test(s)) return "UTC";
  if (fixedOffset(s) !== null) return s;
  try {
    formatter(s);
    return s;
  } catch {
    return null;
  }
}

/** `zone`'s offset in minutes at the instant `t` (epoch ms), from Intl's wall-clock fields; whole seconds, rounded to the minute. */
export function offsetAt(zone: string, t: number): number {
  const at = Math.floor(t / SECOND) * SECOND;
  const parts = formatter(zone).formatToParts(at);
  const get = (type: string): number => Number(parts.find((p) => p.type === type)?.value ?? 0);
  const bc = parts.find((p) => p.type === "era")?.value.startsWith("B") ?? false;
  const local = new Date(0);
  local.setUTCFullYear(bc ? 1 - get("year") : get("year"), get("month") - 1, get("day"));
  local.setUTCHours(get("hour") % 24, get("minute"), get("second"), 0);
  return Math.round((local.getTime() - at) / 60_000);
}

/**
 * The table of `name`'s offsets over [from, to] (zone.schema.json): its first
 * transition at `from`, then each change of offset, found by sampling every
 * {@link STEP_MS} and searching each change down to its second. `name` is
 * `UTC`, a fixed offset or an IANA name (see {@link resolveZone}).
 */
export function zoneTable(name: string, from: number, to: number): ZoneTable {
  const lo = Math.floor(Math.min(from, to));
  const hi = Math.ceil(Math.max(from, to));
  const fixed = name === "UTC" ? 0 : fixedOffset(name);
  if (fixed !== null) return { name, transitions: [{ at: lo, offset: fixed }], covering: [lo, hi] };
  const transitions: Transition[] = [{ at: lo, offset: offsetAt(name, lo) }];
  let prevAt = lo;
  let prev = transitions[0]?.offset ?? 0;
  for (let t = lo; t < hi; ) {
    const next = Math.min(t + STEP_MS, hi);
    let o = offsetAt(name, next);
    while (o !== prev) {
      // The first second in (prevAt, next] whose offset is not `prev`.
      let a = Math.floor(prevAt / SECOND);
      let b = Math.ceil(next / SECOND);
      while (b - a > 1) {
        const mid = Math.floor((a + b) / 2);
        if (offsetAt(name, mid * SECOND) === prev) a = mid;
        else b = mid;
      }
      const at = Math.max(b * SECOND, prevAt + 1);
      prev = offsetAt(name, at);
      transitions.push({ at, offset: prev });
      prevAt = at;
      o = offsetAt(name, next);
    }
    prevAt = next;
    t = next;
  }
  return { name, transitions, covering: [lo, hi] };
}

/** Whether `table` covers [from, to]. */
export function covers(table: ZoneTable | null, from: number, to: number): boolean {
  return table !== null && table.covering[0] <= from && table.covering[1] >= to;
}

/** The span a table is built over for [from, to]: widened by its own width (at least a day) on each side, so a view panned a whole width either way is still covered. */
export function coveringFor(from: number, to: number): [number, number] {
  const pad = Math.max(DAY, to - from);
  return [Math.floor(from - pad), Math.ceil(to + pad)];
}
