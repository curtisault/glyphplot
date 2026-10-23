// Accessibility (REQ-A11Y-010 to 060, ADR-132): the host is a figure named
// by `label`, else `<type>: <series names>`; the drawing is an image whose
// label is the chart's summary, rewritten at most once a second; one tab
// stop, a slider cursor whose `aria-valuetext` reads the cursor's x and up to
// four series' values; a visually hidden light-DOM table of the view, at most
// 500 rows; a control row (zoom out, zoom in, reset, step back, step on) shown
// on focus and hover; reduced motion; a 2 px focus ring in the accent. From
// v0.7.0 a "Show data" toggle in the control row, `T` on the cursor and the
// `show-data` attribute make the table visible (REQ-A11Y-110).

import type { TableJson } from "./api.ts";
import type { DataObject } from "./data.ts";
import { xColumnOf } from "./stream.ts";

/** The keys of the plan's keys table, as `aria-keyshortcuts` lists them (REQ-A11Y-030). */
export const KEY_SHORTCUTS =
  "ArrowLeft ArrowRight ArrowUp ArrowDown Home End PageUp PageDown Shift+ArrowLeft Shift+ArrowRight + = - 0 Enter Shift+Enter Escape Space 1 2 3 4 5 6 7 8 9 ? T";

/** The key on the cursor that shows or hides the table (REQ-A11Y-110); the shell's, the core never sees it. */
export const SHOW_DATA_KEY = "t";

/** The "Show data" toggle's label and glyph (REQ-A11Y-110); `aria-pressed` says whether the table is shown. */
export const SHOW_DATA = { label: "Show data", glyph: "▦" } as const;

/** The control row (REQ-A11Y-040): label, glyph, and the key it sends. */
export const CONTROLS: readonly { label: string; glyph: string; key: string }[] = [
  { label: "Zoom out", glyph: "−", key: "-" },
  { label: "Zoom in", glyph: "+", key: "+" },
  { label: "Reset view", glyph: "↺", key: "0" },
  { label: "Step back", glyph: "◀", key: "ArrowLeft" },
  { label: "Step forward", glyph: "▶", key: "ArrowRight" },
];

/** The most rows the hidden table holds (REQ-A11Y-020). */
export const TABLE_ROWS = 500;
/** The least time between rewrites of the summary and the table, ms (REQ-A11Y-010). */
export const REWRITE_MS = 1000;

/** The least interval of the `announce` live region, seconds (REQ-A11Y-100). */
export const ANNOUNCE_MIN_S = 5;

/** The `announce` attribute's interval in seconds: its number, at least {@link ANNOUNCE_MIN_S}; the least for a value that is not a number. */
export function announceSeconds(v: string | null): number {
  const n = Number(v);
  return v !== null && v.trim() !== "" && Number.isFinite(n) ? Math.max(ANNOUNCE_MIN_S, n) : ANNOUNCE_MIN_S;
}

/**
 * What the live region says of the newest row (REQ-A11Y-100): "Latest: ",
 * the x, then up to four other columns' values, as the slider reads them;
 * "" without a row.
 */
export function latestText(row: Record<string, unknown> | null, desc: DataObject | null, fmt: Formats, encode: Record<string, unknown> = {}): string {
  if (!row) return "";
  const types = columnTypes(desc);
  const table = desc ? Object.values(desc.tables)[0] : undefined;
  const xi = table ? xColumnOf(table, encode) : -1;
  const xName = xi >= 0 ? table?.columns[xi]?.name : undefined;
  const values = Object.entries(row)
    .filter(([k]) => k !== xName)
    .map(([k, v]) => `${k} ${fmt.value(v, types.get(k))}`);
  const shown = values.slice(0, 4);
  if (values.length > 4) shown.push(`and ${values.length - 4} more`);
  const x = xName !== undefined ? fmt.value(row[xName], types.get(xName)) : "";
  return `Latest: ${[x, ...shown].filter((s) => s !== "").join(", ")}`;
}

/** The type of every column by name, from a descriptor. */
function columnTypes(desc: DataObject | null): Map<string, { type: string; zone?: string }> {
  const out = new Map<string, { type: string; zone?: string }>();
  for (const t of Object.values(desc?.tables ?? {})) {
    for (const c of t.columns) out.set(c.name, c.zone === undefined ? { type: c.type } : { type: c.type, zone: c.zone });
  }
  return out;
}

/** Formats values for reading aloud, in the locale; timestamps as dates. */
export class Formats {
  readonly #num: Intl.NumberFormat;
  readonly #count: Intl.NumberFormat;
  readonly #dates = new Map<string, Intl.DateTimeFormat>();
  readonly #locale: string;

  constructor(locale: string) {
    this.#locale = locale;
    let num: Intl.NumberFormat;
    try {
      num = new Intl.NumberFormat(locale, { maximumSignificantDigits: 6 });
    } catch {
      num = new Intl.NumberFormat("en-US", { maximumSignificantDigits: 6 });
    }
    this.#num = num;
    let count: Intl.NumberFormat;
    try {
      count = new Intl.NumberFormat(locale, { maximumFractionDigits: 0 });
    } catch {
      count = new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 });
    }
    this.#count = count;
  }

  /** A count, whole and exact (`number` keeps six significant digits). */
  count(n: number): string {
    return this.#count.format(n);
  }

  number(v: number): string {
    return Number.isFinite(v) ? this.#num.format(v) : v > 0 ? "∞" : v < 0 ? "−∞" : "no value";
  }

  date(ms: number, zone = "UTC"): string {
    let f = this.#dates.get(zone);
    if (!f) {
      try {
        f = new Intl.DateTimeFormat(this.#locale, { dateStyle: "medium", timeStyle: "short", timeZone: zone });
      } catch {
        f = new Intl.DateTimeFormat("en-US", { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" });
      }
      this.#dates.set(zone, f);
    }
    return Number.isFinite(ms) ? f.format(ms) : "no value";
  }

  /** A value as the table and the slider read it. */
  value(v: unknown, type?: { type: string; zone?: string }): string {
    if (v === null || v === undefined) return "no value";
    if (typeof v === "number") return type?.type === "timestamp" ? this.date(v, type.zone) : this.number(v);
    if (typeof v === "boolean") return v ? "true" : "false";
    return String(v);
  }
}

/** A hover detail as plot-hover carries it (events.schema.json). */
interface HoverDetail {
  series?: string;
  x?: number | string;
  y?: number | null;
  datum?: Record<string, unknown>;
}

/**
 * The slider's `aria-valuetext` (REQ-A11Y-030): the cursor's x, then each
 * series' value there, up to four, then "and N more". Built from a
 * plot-hover detail; `desc` says which columns are timestamps.
 */
export function valueText(detail: unknown, desc: DataObject | null, fmt: Formats): string {
  if (detail === null || typeof detail !== "object") return "No point selected";
  const d = detail as HoverDetail;
  const types = columnTypes(desc);
  const entries = Object.entries(d.datum ?? {});
  // The detail does not name the x column: take the datum's key holding x,
  // never the hovered series' own (whose value can equal x, as y = 0 at x = 0
  // does), a timestamp column first.
  const xs = entries.filter(([k, v]) => v === d.x && k !== d.series).map(([k]) => k);
  const xKey = xs.find((k) => types.get(k)?.type === "timestamp") ?? xs[0] ?? entries.find(([, v]) => v === d.x)?.[0];
  const xText = typeof d.x === "string" ? d.x : d.x === undefined ? "" : fmt.value(d.x, xKey ? types.get(xKey) : undefined);
  let values = entries.filter(([k]) => k !== xKey).map(([k, v]) => `${k} ${fmt.value(v, types.get(k))}`);
  if (values.length === 0 && d.series !== undefined) values = [`${d.series} ${fmt.value(d.y ?? null)}`];
  const shown = values.slice(0, 4);
  if (values.length > 4) shown.push(`and ${values.length - 4} more`);
  return [xText, ...shown].filter((s) => s !== "").join(", ");
}

/** The figure's name without `label` (REQ-A11Y-010): `<type>: <series names>`. */
export function figureName(type: string, series: readonly string[]): string {
  return series.length > 0 ? `${type}: ${series.join(", ")}` : type;
}

/**
 * The table's caption (REQ-A11Y-020): the chart's, and the count when the
 * view holds more points than the table, stated once: a chart caption that
 * already gives the total (as the line chart's did before v0.3.0) gets
 * nothing appended.
 */
export function captionOf(t: Pick<TableJson, "caption" | "rows" | "total">, fmt: Formats): string {
  const base = t.caption || "Data in view";
  if (t.total <= t.rows.length) return base;
  const total = fmt.count(t.total);
  if (base.includes(total)) return base;
  return `${base}: ${fmt.count(t.rows.length)} of ${total} points; zoom in for more`;
}

/** The accessible parts of one chart, made in the element's light DOM. */
export interface A11yParts {
  /** The box the drawing and cursor share; pointer listeners go here. */
  frame: HTMLDivElement;
  cursor: HTMLDivElement;
  controls: HTMLDivElement;
  /** The control row's "Show data" toggle (REQ-A11Y-110). */
  showData: HTMLButtonElement;
  tableBox: HTMLDivElement;
}

let boxes = 0;

/** Makes the frame, the slider cursor, the control row with its "Show data" toggle, and the hidden table box. */
export function makeParts(doc: Document, onControl: (key: string) => void, onShowData: () => void = () => {}): A11yParts {
  const frame = doc.createElement("div");
  frame.className = "gp-frame";
  const cursor = doc.createElement("div");
  cursor.className = "gp-cursor";
  cursor.tabIndex = 0;
  cursor.setAttribute("role", "slider");
  cursor.setAttribute("aria-label", "Chart cursor");
  cursor.setAttribute("aria-orientation", "horizontal");
  cursor.setAttribute("aria-keyshortcuts", KEY_SHORTCUTS);
  cursor.setAttribute("aria-valuemin", "0");
  cursor.setAttribute("aria-valuemax", "0");
  cursor.setAttribute("aria-valuenow", "0");
  cursor.setAttribute("aria-valuetext", "No point selected");
  frame.append(cursor);
  const controls = doc.createElement("div");
  controls.className = "gp-controls";
  controls.setAttribute("role", "group");
  controls.setAttribute("aria-label", "Chart controls");
  for (const c of CONTROLS) {
    const b = doc.createElement("button");
    b.type = "button";
    b.tabIndex = -1; // one tab stop: the cursor (REQ-A11Y-030); the keys do the same
    b.textContent = c.glyph;
    b.setAttribute("aria-label", c.label);
    b.title = c.label;
    b.addEventListener("pointerdown", (e) => e.preventDefault()); // keep focus where it is
    b.addEventListener("click", () => onControl(c.key));
    controls.append(b);
  }
  const tableBox = doc.createElement("div");
  tableBox.className = "gp-sr";
  tableBox.id = `gp-table-${++boxes}`;
  const showData = doc.createElement("button");
  showData.type = "button";
  showData.tabIndex = -1; // one tab stop: T on the cursor does the same (REQ-A11Y-030)
  showData.className = "gp-show-data";
  showData.textContent = SHOW_DATA.glyph;
  showData.setAttribute("aria-label", SHOW_DATA.label);
  showData.setAttribute("aria-pressed", "false");
  showData.setAttribute("aria-controls", tableBox.id);
  showData.title = `${SHOW_DATA.label} (T)`;
  showData.addEventListener("pointerdown", (e) => e.preventDefault());
  showData.addEventListener("click", () => onShowData());
  controls.append(showData);
  return { frame, cursor, controls, showData, tableBox };
}

/**
 * Shows or hides the table (REQ-A11Y-110): shown, the box is a labelled
 * region over the drawing that scrolls and takes focus (so the keyboard can
 * scroll it); hidden, it is visually hidden again, still read by screen
 * readers (REQ-A11Y-020). The toggle's `aria-pressed` follows.
 */
export function showTable(parts: Pick<A11yParts, "tableBox" | "showData">, on: boolean): void {
  const box = parts.tableBox;
  box.className = on ? "gp-table" : "gp-sr";
  if (on) {
    box.setAttribute("role", "region");
    box.setAttribute("aria-label", "Chart data");
    box.tabIndex = 0;
  } else {
    box.removeAttribute("role");
    box.removeAttribute("aria-label");
    box.removeAttribute("tabindex");
  }
  parts.showData.setAttribute("aria-pressed", String(on));
}

/** Writes the hidden table: a caption, `th scope="col"` headers and a row header a row (REQ-A11Y-020). */
export function writeTable(box: HTMLElement, t: TableJson, fmt: Formats): void {
  const doc = box.ownerDocument;
  const table = doc.createElement("table");
  table.createCaption().textContent = captionOf(t, fmt);
  const head = table.createTHead().insertRow();
  for (const name of t.columns) {
    const th = doc.createElement("th");
    th.scope = "col";
    th.textContent = name;
    head.append(th);
  }
  const body = table.createTBody();
  for (const row of t.rows.slice(0, TABLE_ROWS)) {
    const tr = body.insertRow();
    row.forEach((v, i) => {
      // The core's text as it wrote it (REQ-FMT-030).
      const text = v ?? fmt.value(null);
      if (i === 0) {
        const th = doc.createElement("th");
        th.scope = "row";
        th.textContent = text;
        tr.append(th);
      } else tr.insertCell().textContent = text;
    });
  }
  box.replaceChildren(table);
}

/** Runs `f` at most once every `ms`, the last call winning; returns the trigger and a cancel. */
export function throttle(f: () => void, ms: number): { run(): void; cancel(): void } {
  let last = -Infinity;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const fire = (): void => {
    timer = null;
    last = Date.now();
    f();
  };
  return {
    run() {
      if (timer !== null) return;
      const wait = last + ms - Date.now();
      if (wait <= 0) fire();
      else timer = setTimeout(fire, wait);
    },
    cancel() {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    },
  };
}
