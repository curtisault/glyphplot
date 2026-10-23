// Arrow IPC in the shell (REQ-DATA-150): only enough of the format to check
// that bytes are an Arrow IPC stream or file and count their rows, for
// `plot-data` (REQ-STREAM-070). The core reads the data itself through the
// module's `set_arrow(bytes)` (ADR-105); the shell never decodes a column.
//
// A stream is a run of messages, each `0xFFFFFFFF`, a little-endian i32
// metadata length, a flatbuffer `Message` padded to 8 bytes, then its body;
// a length of 0 ends it (pre-1.0 writers leave out the `0xFFFFFFFF`). A file
// is `ARROW1`, two bytes of padding, the same stream, a footer and `ARROW1`
// again. `Message` is a flatbuffer table: field 1 `header_type` (a u8, 3 for a
// RecordBatch), field 2 `header`, field 3 `bodyLength` (i64); a
// `RecordBatch`'s field 0 is its `length` (i64) in rows.

const MAGIC = [0x41, 0x52, 0x52, 0x4f, 0x57, 0x31]; // "ARROW1"
const RECORD_BATCH = 3;
const SCHEMA = 1;

class ArrowError extends Error {
  override name = "ArrowError";
}

/** Reads one flatbuffer table's fields by index, bounds-checked against `end`. */
class Table {
  readonly #v: DataView;
  readonly #pos: number;
  readonly #vt: number;
  readonly #vtLen: number;
  constructor(v: DataView, pos: number) {
    if (pos < 0 || pos + 4 > v.byteLength) throw new ArrowError("a table outside the message");
    this.#v = v;
    this.#pos = pos;
    this.#vt = pos - v.getInt32(pos, true);
    if (this.#vt < 0 || this.#vt + 4 > v.byteLength) throw new ArrowError("a vtable outside the message");
    this.#vtLen = v.getUint16(this.#vt, true);
  }
  #field(i: number): number {
    const o = 4 + 2 * i;
    if (o + 2 > this.#vtLen) return 0;
    const at = this.#v.getUint16(this.#vt + o, true);
    return at === 0 ? 0 : this.#pos + at;
  }
  u8(i: number): number {
    const at = this.#field(i);
    return at === 0 ? 0 : this.#v.getUint8(at);
  }
  i64(i: number): number {
    const at = this.#field(i);
    if (at === 0) return 0;
    if (at + 8 > this.#v.byteLength) throw new ArrowError("a field outside the message");
    return Number(this.#v.getBigInt64(at, true));
  }
  table(i: number): Table | null {
    const at = this.#field(i);
    return at === 0 ? null : new Table(this.#v, at + this.#v.getUint32(at, true));
  }
}

/** Whether `b` begins as an Arrow IPC file ("ARROW1"). */
function isFile(b: Uint8Array): boolean {
  return b.length >= 8 && MAGIC.every((m, i) => b[i] === m);
}

/**
 * The rows of an Arrow IPC stream or file: the sum of its record batches'
 * lengths. Throws `not an Arrow IPC stream: …` for bytes that are not one.
 */
export function arrowRows(bytes: Uint8Array): number {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let p = isFile(bytes) ? 8 : 0;
  let rows = 0;
  let schema = false;
  try {
    while (p + 4 <= v.byteLength) {
      let len = v.getInt32(p, true);
      p += 4;
      if (len === -1) {
        if (p + 4 > v.byteLength) break;
        len = v.getInt32(p, true);
        p += 4;
      }
      if (len === 0) break; // end of stream
      if (len < 0 || p + len > v.byteLength) throw new ArrowError(`a message of ${len} bytes at byte ${p - 4} runs past the end`);
      const meta = new DataView(bytes.buffer, bytes.byteOffset + p, len);
      const message = new Table(meta, meta.getUint32(0, true));
      const kind = message.u8(1);
      const body = message.i64(3);
      if (kind === SCHEMA) schema = true;
      if (kind === RECORD_BATCH) rows += message.table(2)?.i64(0) ?? 0;
      p += len + body;
      if (body < 0 || p > v.byteLength) throw new ArrowError(`a message body runs past the end`);
    }
  } catch (e) {
    throw new Error(`not an Arrow IPC stream: ${e instanceof Error ? e.message : String(e)}`);
  }
  if (!schema) throw new Error("not an Arrow IPC stream: no schema message");
  return rows;
}
