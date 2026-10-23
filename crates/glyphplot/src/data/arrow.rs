//! The core's own Arrow IPC stream reader (REQ-DATA-140, ADR-105).
//!
//! [`Table::from_arrow_ipc`] reads an uncompressed Arrow IPC stream, the
//! streaming format (<https://arrow.apache.org/docs/format/Columnar.html>,
//! "Serialization and Interprocess Communication"), into one [`Table`]. The
//! flatbuffers of its messages (`Message.fbs`, `Schema.fbs`) are read by
//! hand, field by field, so no Arrow or flatbuffers crate enters a module
//! (REQ-SIZE-060).
//!
//! What it reads, and the column each becomes (REQ-DATA-010, REQ-DATA-020):
//!
//! | Arrow | Column |
//! |---|---|
//! | Float64, Float32 | `f64`, `f32` |
//! | Int8, Int16, Int32, Int64 | `i64`, held as `f64` (exact to 2^53) |
//! | UInt8, UInt16, UInt32 | `u32` |
//! | Bool | `bool`, 1 or 0 |
//! | Timestamp, each unit | `timestamp` in that unit, its timezone as `zone`; `us` and `ns` as offsets from an `i64` origin |
//! | Utf8, LargeUtf8 | `string` |
//! | Dictionary of Utf8 or LargeUtf8, any integer index | `category`, the dictionary as its labels |
//!
//! A cleared validity bit reads as missing: NaN, an empty string, or
//! [`MISSING_CODE`] (REQ-DATA-030). Record batches concatenate in order.
//! Dictionary batches, deltas and replacements all read: a delta adds its
//! labels to the column's; a replacement adds the labels it has that the
//! column lacks, and the indices of the batches after it map through it.
//! Both framings read: the continuation marker of Arrow 0.15 and later, and
//! the legacy one without it.
//!
//! Anything else is an [`ArrowError`] naming what is wrong and where, never
//! a panic: a truncated stream, the file format, a big-endian or compressed
//! stream, an unsupported type (named, with its column), a buffer past the
//! body.
//!
//! Its errors are fixed text with the place added once (the message's byte,
//! the column), not numbers formatted at each check: the reader is in every
//! chart module, under 8 KB gzipped (REQ-DATA-140).

use std::fmt;

use super::{Column, ColumnType, Table, TimeUnit, MAX_CATEGORIES, MAX_COLUMNS, MISSING_CODE};

/// Why an Arrow IPC stream could not be read: the message names what is
/// wrong and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrowError(pub String);

impl fmt::Display for ArrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "arrow: {}", self.0)
    }
}

impl std::error::Error for ArrowError {}

/// A result whose error says what is wrong; the caller adds where.
type R<T> = Result<T, &'static str>;

/// The names of the `Type` union's members (`Schema.fbs`), for errors.
const TYPES: [&str; 27] = [
    "NONE",
    "Null",
    "Int",
    "FloatingPoint",
    "Binary",
    "Utf8",
    "Bool",
    "Decimal",
    "Date",
    "Time",
    "Timestamp",
    "Interval",
    "List",
    "Struct",
    "Union",
    "FixedSizeBinary",
    "FixedSizeList",
    "Map",
    "Duration",
    "LargeBinary",
    "LargeUtf8",
    "LargeList",
    "RunEndEncoded",
    "BinaryView",
    "Utf8View",
    "ListView",
    "LargeListView",
];

/// One flatbuffer, read by position; every read is bounds-checked and
/// gives `None` past the end.
#[derive(Clone, Copy)]
struct Fb<'a>(&'a [u8]);

impl<'a> Fb<'a> {
    fn get<const N: usize>(self, p: usize) -> Option<[u8; N]> {
        self.0.get(p..p.checked_add(N)?)?.try_into().ok()
    }

    fn u16(self, p: usize) -> Option<u16> {
        self.get(p).map(u16::from_le_bytes)
    }

    fn u32(self, p: usize) -> Option<u32> {
        self.get(p).map(u32::from_le_bytes)
    }

    fn i64(self, p: usize) -> Option<i64> {
        self.get(p).map(i64::from_le_bytes)
    }

    /// Where field `i` of the table at `t` is stored, when present.
    fn field(self, t: usize, i: usize) -> Option<usize> {
        let vt = usize::try_from(t as i64 - self.u32(t)? as i32 as i64).ok()?;
        if 6 + 2 * i > self.u16(vt)? as usize {
            return None;
        }
        match self.u16(vt + 4 + 2 * i)? {
            0 => None,
            o => t.checked_add(o as usize),
        }
    }

    /// The position the offset stored at `p` points to.
    fn deref(self, p: usize) -> Option<usize> {
        p.checked_add(self.u32(p)? as usize)
    }

    /// The table, vector or string field `i` of `t` points to.
    fn table(self, t: usize, i: usize) -> Option<usize> {
        self.deref(self.field(t, i)?)
    }

    /// Scalar field `i` of `t`, its default 0 when absent.
    fn byte(self, t: usize, i: usize) -> u8 {
        self.field(t, i).and_then(|p| self.0.get(p).copied()).unwrap_or(0)
    }

    fn short(self, t: usize, i: usize) -> u16 {
        self.field(t, i).and_then(|p| self.u16(p)).unwrap_or(0)
    }

    fn int(self, t: usize, i: usize) -> u32 {
        self.field(t, i).and_then(|p| self.u32(p)).unwrap_or(0)
    }

    fn long(self, t: usize, i: usize) -> i64 {
        self.field(t, i).and_then(|p| self.i64(p)).unwrap_or(0)
    }

    /// Vector field `i` of `t`: where its items start and how many there
    /// are, `stride` bytes each, all within the buffer; empty when absent.
    fn vec(self, t: usize, i: usize, stride: usize) -> Option<(usize, usize)> {
        let Some(v) = self.table(t, i) else { return Some((0, 0)) };
        let n = self.u32(v)? as usize;
        let start = v + 4;
        (start.checked_add(n.checked_mul(stride)?)? <= self.0.len()).then_some((start, n))
    }

    /// String field `i` of `t`.
    fn str(self, t: usize, i: usize) -> Option<&'a str> {
        let s = self.table(t, i)?;
        let n = self.u32(s)? as usize;
        std::str::from_utf8(self.0.get(s.checked_add(4)?..)?.get(..n)?).ok()
    }
}

/// A non-negative stream integer as a `usize`.
fn size(v: i64) -> R<usize> {
    usize::try_from(v).map_err(|_| "a length, offset or count is negative or too large")
}

/// The `n` bytes at `*p`, moving `*p` past them.
fn take<'a>(bytes: &'a [u8], p: &mut usize, n: usize) -> Option<&'a [u8]> {
    let s = bytes.get(*p..p.checked_add(n)?)?;
    *p += n;
    Some(s)
}

/// A column's Arrow type, as far as the reader takes it.
#[derive(Clone, Copy)]
enum Kind {
    /// FloatingPoint: 4 or 8 bytes.
    Float(usize),
    /// Int: bytes, signed.
    Int(usize, bool),
    Bool,
    Time(TimeUnit),
    /// Utf8 (4-byte offsets) or LargeUtf8 (8).
    Utf8(usize),
    /// Dictionary: its slot in the reader's dictionaries, and its index's bytes and sign.
    Dict(usize, usize, bool),
}

/// A schema field and the rows read for it so far.
struct Field {
    name: String,
    kind: Kind,
    zone: Option<String>,
    nums: Vec<f64>,
    stamps: Vec<Option<i64>>,
    texts: Vec<String>,
    codes: Vec<u32>,
}

/// A dictionary: every label it has given its columns, and the code of
/// each position of the current one.
struct Dict {
    id: i64,
    /// Offset bytes of its Utf8 values: 4, or 8 for LargeUtf8.
    width: usize,
    labels: Vec<String>,
    map: Vec<u32>,
}

/// An integer Arrow type (`Int` table at `t`; absent is a dictionary's
/// default index, Int32): bytes and sign.
fn int_type(fb: Fb, t: Option<usize>) -> R<(usize, bool)> {
    let Some(t) = t else { return Ok((4, true)) };
    match (fb.int(t, 0), fb.byte(t, 1) != 0) {
        (64, false) => Err("UInt64 is not read: a column takes u32 or i64"),
        (w @ (8 | 16 | 32 | 64), s) => Ok((w as usize / 8, s)),
        _ => Err("an integer of a width Arrow does not have"),
    }
}

/// The kind and zone of the Field table at `f`, of type `tt`, its
/// dictionary added to `dicts`; `Err("")` for a type the reader does not take.
fn kind(fb: Fb, f: usize, tt: u8, dicts: &mut Vec<Dict>) -> R<(Kind, Option<String>)> {
    let ty = fb.table(f, 3);
    let mut zone = None;
    let mut kind = match (tt, ty) {
        (2, _) => {
            let (w, s) = int_type(fb, ty)?;
            Kind::Int(w, s)
        }
        (3, Some(ty)) => match fb.short(ty, 0) {
            1 => Kind::Float(4),
            2 => Kind::Float(8),
            _ => return Err("half floats are not read"),
        },
        (5, _) => Kind::Utf8(4),
        (20, _) => Kind::Utf8(8),
        (6, _) => Kind::Bool,
        (10, Some(ty)) => {
            zone = fb.str(ty, 1).map(str::to_string);
            Kind::Time([TimeUnit::S, TimeUnit::Ms, TimeUnit::Us, TimeUnit::Ns][fb.short(ty, 0).min(3) as usize])
        }
        _ => return Err(""),
    };
    if let Some(d) = fb.table(f, 4) {
        let Kind::Utf8(width) = kind else { return Err("only dictionaries of strings are read") };
        let (w, s) = int_type(fb, fb.table(d, 1))?;
        let id = fb.long(d, 0);
        let slot = dicts.iter().position(|d| d.id == id).unwrap_or_else(|| {
            dicts.push(Dict { id, width, labels: Vec::new(), map: Vec::new() });
            dicts.len() - 1
        });
        kind = Kind::Dict(slot, w, s);
    }
    Ok((kind, zone))
}

/// The fields of the Schema table at `h`, their dictionaries added to `dicts`.
fn schema(fb: Fb, h: usize, dicts: &mut Vec<Dict>) -> Result<Vec<Field>, String> {
    if fb.short(h, 0) != 0 {
        return Err("the stream is big-endian".into());
    }
    let (start, n) = fb.vec(h, 1, 4).ok_or("the schema's fields lie past it")?;
    if n > MAX_COLUMNS {
        return Err("the schema has more than 256 columns".into());
    }
    let mut fields = Vec::with_capacity(n);
    for k in 0..n {
        let f = fb.deref(start + 4 * k).ok_or("a field lies past the schema")?;
        let name = fb.str(f, 0).unwrap_or_default().to_string();
        let tt = fb.byte(f, 2);
        let (kind, zone) = match kind(fb, f, tt, dicts) {
            Ok(k) => k,
            Err("") => return Err(format!("column `{name}` is {}, which is not read", TYPES.get(tt as usize).unwrap_or(&"of no known type"))),
            Err(m) => return Err(format!("column `{name}`: {m}")),
        };
        fields.push(Field { name, kind, zone, nums: Vec::new(), stamps: Vec::new(), texts: Vec::new(), codes: Vec::new() });
    }
    Ok(fields)
}

/// A node's rows, validity bitmap (`None`: every row valid) and values buffer.
type Fixed<'a> = (usize, Option<&'a [u8]>, &'a [u8]);

/// A RecordBatch table and its body, read node by node and buffer by buffer.
struct Batch<'a> {
    fb: Fb<'a>,
    body: &'a [u8],
    rows: usize,
    nodes: (usize, usize),
    buffers: (usize, usize),
    node: usize,
    buffer: usize,
}

impl<'a> Batch<'a> {
    fn new(fb: Fb<'a>, rb: usize, body: &'a [u8]) -> R<Batch<'a>> {
        if let Some(c) = fb.table(rb, 3) {
            return Err(match fb.byte(c, 0) {
                0 => "a record batch is compressed with LZ4_FRAME; write the stream uncompressed",
                _ => "a record batch is compressed with ZSTD; write the stream uncompressed",
            });
        }
        Ok(Batch {
            fb,
            body,
            rows: size(fb.long(rb, 0))?,
            nodes: fb.vec(rb, 1, 16).ok_or("the field nodes lie past the message")?,
            buffers: fb.vec(rb, 2, 16).ok_or("the buffers lie past the message")?,
            node: 0,
            buffer: 0,
        })
    }

    /// The next field node's length, checked against the batch's, and null count.
    fn node(&mut self) -> R<(usize, usize)> {
        if self.node >= self.nodes.1 {
            return Err("the record batch has fewer field nodes than the schema has columns");
        }
        let p = self.nodes.0 + 16 * self.node;
        self.node += 1;
        let n = size(self.fb.i64(p).unwrap_or(-1))?;
        if n != self.rows {
            return Err("a field node's length differs from its record batch's");
        }
        Ok((n, size(self.fb.i64(p + 8).unwrap_or(-1))?))
    }

    /// The next buffer, which must lie in the body and hold at least `need` bytes.
    fn buffer(&mut self, need: usize) -> R<&'a [u8]> {
        if self.buffer >= self.buffers.1 {
            return Err("the record batch has fewer buffers than its columns need");
        }
        let p = self.buffers.0 + 16 * self.buffer;
        self.buffer += 1;
        let mut off = size(self.fb.i64(p).unwrap_or(-1))?;
        let b = take(self.body, &mut off, size(self.fb.i64(p + 8).unwrap_or(-1))?).ok_or("a buffer lies past the message's body")?;
        if b.len() < need {
            return Err("a buffer is shorter than its rows need");
        }
        Ok(b)
    }

    /// The next node's rows and validity bitmap, then its values buffer of
    /// `width` bytes a row (`None`: 1 bit a row).
    fn fixed(&mut self, width: Option<usize>) -> R<Fixed<'a>> {
        let (n, nulls) = self.node()?;
        let bits = n.div_ceil(8);
        let valid = self.buffer(0)?;
        let valid = match nulls {
            0 => None,
            _ if valid.len() < bits => return Err("a validity bitmap is shorter than its rows"),
            _ => Some(valid),
        };
        let need = width.map_or(Some(bits), |w| n.checked_mul(w)).ok_or("more rows than can be held")?;
        Ok((n, valid, self.buffer(need)?))
    }

    /// The next Utf8 or LargeUtf8 node (`width`-byte offsets): each row's
    /// text, `None` where missing.
    fn texts(&mut self, width: usize) -> R<Vec<Option<&'a str>>> {
        let (n, valid, offsets) = self.fixed(Some(width))?;
        // The offsets buffer has one entry more than the rows.
        if offsets.len() < (n + 1) * width {
            return Err("a string column's offsets are fewer than its rows");
        }
        let data = self.buffer(0)?;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let at = |i: usize| usize::try_from(int(&offsets[i * width..(i + 1) * width], true)).ok();
            out.push(match is_valid(valid, i) {
                false => None,
                true => {
                    let s = at(i).zip(at(i + 1)).and_then(|(a, b)| data.get(a..b)).ok_or("a string's offsets lie outside its data")?;
                    Some(std::str::from_utf8(s).map_err(|_| "a string is not UTF-8")?)
                }
            });
        }
        Ok(out)
    }
}

/// Whether bit `i` of a validity bitmap is set; every row is without one.
fn is_valid(valid: Option<&[u8]>, i: usize) -> bool {
    valid.is_none_or(|v| v[i >> 3] >> (i & 7) & 1 == 1)
}

/// A little-endian integer of 1 to 8 bytes, sign-extended when `signed`.
fn int(b: &[u8], signed: bool) -> i64 {
    // The common width read at once.
    if let Ok(x) = b.try_into() {
        return i64::from_le_bytes(x);
    }
    let mut x = [0u8; 8];
    x.iter_mut().zip(b).for_each(|(d, s)| *d = *s);
    let shift = 64 - 8 * b.len() as u32;
    let v = i64::from_le_bytes(x);
    if signed && shift > 0 {
        (v << shift) >> shift
    } else {
        v
    }
}

/// One record batch's rows of field `f`, appended to it.
fn rows(b: &mut Batch, f: &mut Field, dicts: &[Dict]) -> R<()> {
    let w = match f.kind {
        Kind::Utf8(w) => {
            f.texts.extend(b.texts(w)?.into_iter().map(|s| s.unwrap_or_default().to_string()));
            return Ok(());
        }
        Kind::Float(w) | Kind::Int(w, _) | Kind::Dict(_, w, _) => w,
        Kind::Time(_) => 8,
        Kind::Bool => 0,
    };
    let (n, valid, v) = b.fixed((w > 0).then_some(w))?;
    for i in 0..n {
        let ok = is_valid(valid, i);
        let c = &v[i * w..(i + 1) * w];
        let num = |x: f64| if ok { x } else { f64::NAN };
        match f.kind {
            Kind::Float(4) => f.nums.push(num(f32::from_bits(int(c, false) as u32) as f64)),
            Kind::Float(_) => f.nums.push(num(f64::from_bits(int(c, false) as u64))),
            Kind::Int(_, s) => f.nums.push(num(int(c, s) as f64)),
            Kind::Bool => f.nums.push(num((v[i >> 3] >> (i & 7) & 1) as f64)),
            Kind::Time(_) => f.stamps.push(ok.then(|| int(c, true))),
            Kind::Dict(slot, _, s) => f.codes.push(match ok {
                false => MISSING_CODE,
                true => *usize::try_from(int(c, s)).ok().and_then(|k| dicts[slot].map.get(k)).ok_or("a dictionary index is past its dictionary")?,
            }),
            Kind::Utf8(_) => {}
        }
    }
    Ok(())
}

/// A DictionaryBatch (table `h`) read into `dicts`: a delta extends the
/// current dictionary, any other replaces it.
fn dictionary(fb: Fb, h: usize, body: &[u8], dicts: &mut [Dict]) -> R<()> {
    let id = fb.long(h, 0);
    let d = dicts.iter_mut().find(|d| d.id == id).ok_or("a dictionary batch for a dictionary no column uses")?;
    let rb = fb.table(h, 1).ok_or("a dictionary batch without data")?;
    let texts = Batch::new(fb, rb, body)?.texts(d.width)?;
    // A first dictionary's labels and a delta's are new; a replacement's may be known.
    let search = fb.byte(h, 2) == 0 && !d.labels.is_empty();
    if fb.byte(h, 2) == 0 {
        d.map.clear();
    }
    for (k, t) in texts.into_iter().enumerate() {
        let code = match t {
            None => MISSING_CODE,
            Some(t) => {
                // Where it was before, else anywhere, else a new label.
                let known = search.then(|| if d.labels.get(k).is_some_and(|l| l == t) { Some(k) } else { d.labels.iter().position(|l| l == t) });
                match known.flatten() {
                    Some(c) => c as u32,
                    None if d.labels.len() >= MAX_CATEGORIES => return Err("a dictionary has more than 65,535 labels"),
                    None => {
                        d.labels.push(t.to_string());
                        (d.labels.len() - 1) as u32
                    }
                }
            }
        };
        d.map.push(code);
    }
    Ok(())
}

/// The stream read so far: the schema's fields with their rows, and the dictionaries.
struct Stream {
    fields: Option<Vec<Field>>,
    dicts: Vec<Dict>,
}

impl Stream {
    /// Reads the message at `*p` and moves `*p` past it; `false` at the end-of-stream marker.
    fn message(&mut self, bytes: &[u8], p: &mut usize) -> Result<bool, String> {
        let s = Fb(bytes);
        let mut len = s.u32(*p).ok_or("the stream ends inside it")?;
        *p += 4;
        if len == u32::MAX {
            len = s.u32(*p).ok_or("the stream ends inside it")?;
            *p += 4;
        }
        if len == 0 {
            return Ok(false);
        }
        let meta = Fb(take(bytes, p, len as usize).ok_or("its metadata runs past the end: the stream is truncated, or not an Arrow IPC stream")?);
        let msg = meta.deref(0).ok_or("its metadata is not a flatbuffer")?;
        if meta.short(msg, 0) < 3 {
            return Err("its metadata version is older than V4".into());
        }
        let body = take(bytes, p, size(meta.long(msg, 3))?).ok_or("its body runs past the end: the stream is truncated")?;
        let header = meta.table(msg, 2).ok_or("it has no header")?;
        match (meta.byte(msg, 1), &mut self.fields) {
            (1, None) => self.fields = Some(schema(meta, header, &mut self.dicts)?),
            (1, Some(_)) => return Err("a second schema".into()),
            (2 | 3, None) => return Err("a batch before the schema".into()),
            (2, Some(_)) => dictionary(meta, header, body, &mut self.dicts)?,
            (3, Some(fields)) => {
                let mut b = Batch::new(meta, header, body)?;
                for f in fields.iter_mut() {
                    rows(&mut b, f, &self.dicts).map_err(|m| format!("column `{}`: {m}", f.name))?;
                }
            }
            _ => return Err("it is not a schema, dictionary or record batch".into()),
        }
        Ok(true)
    }
}

impl Table {
    /// The table an uncompressed Arrow IPC stream holds: its schema's
    /// columns, every record batch's rows in order (REQ-DATA-140, ADR-105;
    /// the types and how each is held are in the module's documentation).
    /// Bad input is an error naming what is wrong, never a panic.
    pub fn from_arrow_ipc(bytes: &[u8]) -> Result<Table, ArrowError> {
        if bytes.starts_with(b"ARROW1") {
            return Err(ArrowError("this is the Arrow file format; send the stream format".into()));
        }
        let mut s = Stream { fields: None, dicts: Vec::new() };
        let mut p = 0;
        // A stream may end at a message boundary without its end-of-stream marker.
        while p < bytes.len() {
            let at = p;
            match s.message(bytes, &mut p) {
                Ok(true) => {}
                Ok(false) => break,
                Err(m) => return Err(ArrowError(format!("the message at byte {at}: {m}"))),
            }
        }
        let fields = s.fields.ok_or_else(|| ArrowError("no schema: the stream is empty, or not an Arrow IPC stream".into()))?;
        let columns = fields
            .into_iter()
            .map(|f| {
                let n = &f.name;
                let c = match f.kind {
                    Kind::Float(w) => Column::numbers(n, if w == 4 { ColumnType::F32 } else { ColumnType::F64 }, f.nums),
                    Kind::Int(_, s) => Column::numbers(n, if s { ColumnType::I64 } else { ColumnType::U32 }, f.nums),
                    Kind::Bool => Column::numbers(n, ColumnType::Bool, f.nums),
                    Kind::Time(unit) => Column::timestamps(n, unit, &f.stamps),
                    Kind::Utf8(_) => Column::strings(n, f.texts),
                    Kind::Dict(slot, ..) => Column::categories(n, s.dicts[slot].labels.clone(), f.codes),
                };
                Column { zone: f.zone, ..c }
            })
            .collect();
        Table::from_columns(columns).map_err(|e| ArrowError(e.to_string()))
    }
}
