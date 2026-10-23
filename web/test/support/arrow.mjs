// A small Arrow IPC stream writer for the tests (REQ-DATA-150): float64,
// int64 and millisecond-timestamp columns without nulls, one record batch,
// laid out by the Arrow columnar format's IPC spec
// (https://arrow.apache.org/docs/format/Columnar.html, "Serialization and
// Interprocess Communication") over a flatbuffer written front to back, every
// offset pointing forward. Checked against pyarrow 21 reading its output
// (uv run --with pyarrow==21.0.0). The site's build serves its output as
// data/walk.arrows (site/vite.config.ts).

const SIZE = { u8: 1, bool: 1, i16: 2, i32: 4, off: 4, i64: 8 };

class Out {
  constructor() {
    this.b = new Uint8Array(256);
    this.v = new DataView(this.b.buffer);
    this.n = 0;
  }
  room(k) {
    if (this.n + k <= this.b.length) return;
    const b = new Uint8Array(Math.max(this.b.length * 2, this.n + k));
    b.set(this.b);
    this.b = b;
    this.v = new DataView(b.buffer);
  }
  pad(a) {
    while (this.n % a) this.put("u8", 0);
  }
  put(t, x) {
    this.room(8);
    const at = this.n;
    if (t === "u8" || t === "bool") this.v.setUint8(at, Number(x));
    else if (t === "i16") this.v.setInt16(at, x, true);
    else if (t === "i32") this.v.setInt32(at, x, true);
    else if (t === "u32" || t === "off") this.v.setUint32(at, x, true);
    else if (t === "i64") this.v.setBigInt64(at, BigInt(x), true);
    this.n += t === "u32" ? 4 : SIZE[t];
    return at;
  }
  bytes(u8) {
    this.room(u8.length);
    this.b.set(u8, this.n);
    this.n += u8.length;
  }
  done() {
    return this.b.slice(0, this.n);
  }
}

/** A table: `fields[i]` is `[type, value]` or undefined; type `off` takes a child object. */
const table = (...fields) => ({ kind: "table", fields });
/** A vector of tables. */
const tables = (items) => ({ kind: "tables", items });
/** A vector of 16-byte structs of two i64. */
const pairs = (items) => ({ kind: "pairs", items });
const string = (s) => ({ kind: "string", s });

function write(o, obj) {
  if (obj.kind === "string") {
    o.pad(4);
    const at = o.put("u32", 0);
    const u8 = new TextEncoder().encode(obj.s);
    o.v.setUint32(at, u8.length, true);
    o.bytes(u8);
    o.put("u8", 0);
    return at;
  }
  if (obj.kind === "pairs") {
    while ((o.n + 4) % 8) o.put("u8", 0);
    const at = o.put("u32", obj.items.length);
    for (const [a, b] of obj.items) {
      o.put("i64", a);
      o.put("i64", b);
    }
    return at;
  }
  if (obj.kind === "tables") {
    o.pad(4);
    const at = o.put("u32", obj.items.length);
    const slots = obj.items.map(() => o.put("u32", 0));
    obj.items.forEach((child, i) => {
      const c = write(o, child);
      o.v.setUint32(slots[i], c - slots[i], true);
    });
    return at;
  }
  // A table: its vtable, then the table 8-aligned, then its children.
  const present = obj.fields.map((f, i) => (f ? { i, t: f[0], x: f[1] } : null)).filter(Boolean);
  const order = [...present].sort((a, b) => SIZE[b.t] - SIZE[a.t]);
  let cur = 4;
  for (const f of order) {
    cur = Math.ceil(cur / SIZE[f.t]) * SIZE[f.t];
    f.at = cur;
    cur += SIZE[f.t];
  }
  const size = Math.ceil(cur / 4) * 4;
  o.pad(2);
  const vt = o.n;
  o.put("i16", 4 + 2 * obj.fields.length);
  o.put("i16", size);
  for (let i = 0; i < obj.fields.length; i++) o.put("i16", present.find((f) => f.i === i)?.at ?? 0);
  o.pad(8);
  const start = o.n;
  o.put("i32", start - vt);
  while (o.n < start + size) o.put("u8", 0);
  const kids = [];
  for (const f of present) {
    if (f.t === "off") kids.push(f);
    else {
      const keep = o.n;
      o.n = start + f.at;
      o.put(f.t, f.x);
      o.n = keep;
    }
  }
  for (const f of kids) {
    const c = write(o, f.x);
    o.v.setUint32(start + f.at, c - (start + f.at), true);
  }
  return start;
}

function flatbuffer(root) {
  const o = new Out();
  o.put("u32", 0);
  const at = write(o, root);
  o.v.setUint32(0, at, true);
  return o.done();
}

const TYPE = { int: 2, float: 3, timestamp: 10 };
const HEADER = { schema: 1, recordBatch: 3 };
const V5 = 4;

function field(name, type) {
  const t =
    type === "f64"
      ? [TYPE.float, table(["i16", 2])]
      : type === "i64"
        ? [TYPE.int, table(["i32", 64], ["bool", 1])]
        : [TYPE.timestamp, table(["i16", 1], ["off", string("UTC")])];
  return table(["off", string(name)], ["bool", 0], ["u8", t[0]], ["off", t[1]], undefined, ["off", tables([])]);
}

/** One encapsulated message: continuation, length, the flatbuffer padded to 8, then the body. */
function message(header, kind, body) {
  const fb = flatbuffer(table(["i16", V5], ["u8", kind], ["off", header], ["i64", body.length]));
  const len = Math.ceil((fb.length + 8) / 8) * 8 - 8;
  const out = new Uint8Array(8 + len + body.length);
  const v = new DataView(out.buffer);
  v.setUint32(0, 0xffffffff, true);
  v.setInt32(4, len, true);
  out.set(fb, 8);
  out.set(body, 8 + len);
  return out;
}

/**
 * An Arrow IPC stream of one record batch. `columns` maps a name to
 * `{type: "f64" | "i64" | "timestamp", values}` (timestamps in ms since the
 * epoch, UTC); every column the same length, none with nulls.
 */
export function arrowStream(columns) {
  const names = Object.keys(columns);
  const n = columns[names[0]].values.length;
  const schema = message(table(undefined, ["off", tables(names.map((k) => field(k, columns[k].type)))]), HEADER.schema, new Uint8Array(0));
  const buffers = [];
  const parts = [];
  let offset = 0;
  for (const k of names) {
    const { type, values } = columns[k];
    const data = type === "f64" ? Float64Array.from(values) : BigInt64Array.from(values, (x) => BigInt(Math.round(x)));
    const bytes = new Uint8Array(data.buffer);
    buffers.push([offset, 0], [offset, bytes.length]);
    parts.push(bytes);
    offset += Math.ceil(bytes.length / 8) * 8;
  }
  const body = new Uint8Array(offset);
  let p = 0;
  for (const b of parts) {
    body.set(b, p);
    p += Math.ceil(b.length / 8) * 8;
  }
  const batch = message(table(["i64", n], ["off", pairs(names.map(() => [n, 0]))], ["off", pairs(buffers)]), HEADER.recordBatch, body);
  const end = Uint8Array.of(0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0);
  const out = new Uint8Array(schema.length + batch.length + end.length);
  out.set(schema, 0);
  out.set(batch, schema.length);
  out.set(end, schema.length + batch.length);
  return out;
}

/**
 * An Arrow IPC file framing {@link arrowStream}'s stream: "ARROW1", two bytes
 * of padding, the stream with its end-of-stream marker, a footer, its i32
 * length and "ARROW1". The footer is 8 zero bytes, not a flatbuffer: neither
 * the shell nor the core reads past the end-of-stream marker.
 */
export function arrowFile(columns) {
  const stream = arrowStream(columns);
  const magic = [0x41, 0x52, 0x52, 0x4f, 0x57, 0x31];
  const out = new Uint8Array(8 + stream.length + 8 + 4 + 6);
  out.set(magic, 0);
  out.set(stream, 8);
  new DataView(out.buffer).setInt32(8 + stream.length + 8, 8, true);
  out.set(magic, out.length - 6);
  return out;
}

/** The walk the site serves as data/walk.arrows and as walk.csv, .json and .ndjson: `n` rows of t (ms from 2026-01-01) and y. */
export function walk(n = 50) {
  const t = [];
  const y = [];
  let w = 10;
  for (let i = 0; i < n; i++) {
    t.push(Date.UTC(2026, 0, 1) + i * 60_000);
    w += Math.sin(i * 1.7) * 0.5;
    y.push(Math.round(w * 100) / 100);
  }
  return { t, y };
}
