// `src` (REQ-DATA-150): a URL of CSV, JSON, NDJSON or Arrow, fetched once per
// change and parsed in the shell. The format comes from the response's
// Content-Type, else the URL's extension. CSV is read by the small reader
// here: a header row, fields quoted with `"` (a doubled `""` inside one),
// CRLF or LF, every column typed by its content (numbers, `true`/`false`,
// else text, which the columns shorthand reads as timestamps when every
// value is an RFC 3339 date-time and as categories otherwise), an empty
// field missing. JSON is any form `data` takes; NDJSON one row object a
// line. Arrow bytes are handed on as they are, for the module's `set_arrow`.
// Pure but for `fetch`: no DOM, no wasm.

import type { Row, Scalar } from "./data.ts";

/** The formats `src` reads. */
export type SourceFormat = "csv" | "json" | "ndjson" | "arrow";

/** What a `src` URL gave: data for `data`, or Arrow bytes. */
export type SourceData = { format: "csv" | "json" | "ndjson"; data: Record<string, Scalar[]> | Row[] | unknown } | { format: "arrow"; bytes: Uint8Array };

const TYPES: Record<string, SourceFormat> = {
  "text/csv": "csv",
  "application/csv": "csv",
  "text/comma-separated-values": "csv",
  "application/json": "json",
  "text/json": "json",
  "application/x-ndjson": "ndjson",
  "application/ndjson": "ndjson",
  "application/jsonl": "ndjson",
  "application/x-jsonlines": "ndjson",
  "application/jsonlines": "ndjson",
  "text/x-ndjson": "ndjson",
  "application/vnd.apache.arrow.stream": "arrow",
  "application/vnd.apache.arrow.file": "arrow",
  "application/x-apache-arrow-stream": "arrow",
};

const EXTENSIONS: Record<string, SourceFormat> = {
  csv: "csv",
  json: "json",
  ndjson: "ndjson",
  jsonl: "ndjson",
  arrow: "arrow",
  arrows: "arrow",
  feather: "arrow",
  ipc: "arrow",
};

/**
 * The format of a response: its Content-Type's media type when it names one
 * of the four (a `+json` suffix is JSON), else the URL path's extension; null
 * when neither says.
 */
export function formatOf(contentType: string | null, url: string): SourceFormat | null {
  const media = (contentType ?? "").split(";")[0]?.trim().toLowerCase() ?? "";
  const byType = TYPES[media] ?? (media.endsWith("+json") ? "json" : undefined);
  if (byType) return byType;
  let path = url;
  try {
    path = new URL(url, "http://x/").pathname;
  } catch {
    // a bare path
  }
  const ext = /\.([A-Za-z0-9]+)$/.exec(path)?.[1]?.toLowerCase();
  return ext ? (EXTENSIONS[ext] ?? null) : null;
}

const NUMBER = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/;

/** The fields of CSV text, row by row: quoted fields, `""` inside quotes, CRLF or LF, a leading BOM dropped, a final newline optional. */
export function csvRows(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  let i = text.charCodeAt(0) === 0xfeff ? 1 : 0;
  const n = text.length;
  let fieldStart = true;
  while (i < n) {
    const ch = text[i] as string;
    if (quoted) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 2;
          continue;
        }
        quoted = false;
        i++;
        continue;
      }
      field += ch;
      i++;
      continue;
    }
    if (ch === '"' && fieldStart) {
      quoted = true;
      fieldStart = false;
      i++;
      continue;
    }
    if (ch === ",") {
      row.push(field);
      field = "";
      fieldStart = true;
      i++;
      continue;
    }
    if (ch === "\n" || ch === "\r") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      fieldStart = true;
      i += ch === "\r" && text[i + 1] === "\n" ? 2 : 1;
      continue;
    }
    field += ch;
    fieldStart = false;
    i++;
  }
  if (quoted) throw new Error(`csv: a quoted field is not closed (row ${rows.length + 1})`);
  if (field !== "" || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  // Blank lines (a lone empty field) are not rows.
  return rows.filter((r) => !(r.length === 1 && r[0] === ""));
}

/**
 * CSV text as the columns shorthand: the first row names the columns, each
 * column typed by its content: numbers when every present field is one,
 * booleans when every one is `true` or `false`, else text. An empty field is
 * missing (null); a row shorter than the header is missing at its end; a
 * longer one, or a header naming a column twice or not at all, is an error.
 */
export function parseCsv(text: string): Record<string, Scalar[]> {
  const rows = csvRows(text);
  const head = rows.shift();
  if (!head) throw new Error("csv: no header row");
  const names = head.map((h) => h.trim());
  names.forEach((name, j) => {
    if (name === "") throw new Error(`csv: column ${j + 1} has no name`);
    if (names.indexOf(name) !== j) throw new Error(`csv: column \`${name}\` is named twice`);
  });
  const out: Record<string, Scalar[]> = {};
  names.forEach((name, j) => {
    const raw = rows.map((r, i) => {
      if (r.length > names.length) throw new Error(`csv: row ${i + 2} has ${r.length} fields, the header ${names.length}`);
      return r[j] ?? "";
    });
    const present = raw.filter((f) => f !== "");
    const numbers = present.every((f) => NUMBER.test(f.trim()));
    const bools = !numbers && present.every((f) => f === "true" || f === "false");
    out[name] = raw.map((f) => (f === "" ? null : numbers ? Number(f) : bools ? f === "true" : f));
  });
  return out;
}

/** NDJSON text as rows: one JSON object a non-blank line. */
export function parseNdjson(text: string): Row[] {
  const rows: Row[] = [];
  text.split(/\r?\n/).forEach((line, i) => {
    if (line.trim() === "") return;
    let v: unknown;
    try {
      v = JSON.parse(line);
    } catch (e) {
      throw new Error(`ndjson: line ${i + 1}: ${e instanceof Error ? e.message : String(e)}`);
    }
    if (v === null || typeof v !== "object" || Array.isArray(v)) throw new Error(`ndjson: line ${i + 1} is not an object`);
    rows.push(v as Row);
  });
  return rows;
}

/** A response's body read as its format says. */
export async function readResponse(r: Response, url: string): Promise<SourceData> {
  if (!r.ok) throw new Error(`HTTP ${r.status}${r.statusText ? ` ${r.statusText}` : ""}`);
  // A host's fallback page for a missing file is not data, whatever the URL's extension.
  if (/^\s*text\/html\b/i.test(r.headers.get("content-type") ?? "")) throw new Error("the server sent an HTML page (text/html), not CSV, JSON, NDJSON or Arrow");
  const format = formatOf(r.headers.get("content-type"), r.url || url);
  if (format === null) throw new Error(`the format is not known from the Content-Type (${r.headers.get("content-type") ?? "none"}) or the extension: use .csv, .json, .ndjson or .arrow`);
  if (format === "arrow") return { format, bytes: new Uint8Array(await r.arrayBuffer()) };
  const text = await r.text();
  if (format === "csv") return { format, data: parseCsv(text) };
  if (format === "ndjson") return { format, data: parseNdjson(text) };
  return { format, data: JSON.parse(text) as unknown };
}

/** Fetches `url` (relative to `base`) and reads it (REQ-DATA-150). Rejects with `src "<url>": …`. */
export async function loadSource(url: string, base?: string, f: typeof fetch = fetch): Promise<SourceData> {
  const href = base ? new URL(url, base).href : url;
  try {
    return await readResponse(await f(href), href);
  } catch (e) {
    throw new Error(`src "${url}": ${e instanceof Error ? e.message : String(e)}`);
  }
}
