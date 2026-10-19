//! Pure Rust: parsers, a ring buffer, downsamplers and planet positions.
//! Used natively by src/bin/native.rs and in wasm by src/lib.rs.

/// A small seeded generator (mulberry32), so native and JS make the same data.
pub struct Mulberry32(pub u32);
impl Mulberry32 {
    /// The next value in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64) / 4_294_967_296.0
    }
}

/// `n` rows of a random walk: t in epoch ms one second apart from 2026-10-18, v the walk.
pub fn walk(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut r = Mulberry32(7);
    let mut t = Vec::with_capacity(n);
    let mut v = Vec::with_capacity(n);
    let mut y = 100.0;
    for i in 0..n {
        y += r.next_f64() - 0.5;
        t.push(1_792_281_600_000.0 + i as f64 * 1000.0);
        v.push(y);
    }
    (t, v)
}

// ---------------------------------------------------------------- CSV

/// Days from 1970-01-01 to a civil date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// An RFC 3339 date-time (`2026-10-18T12:00:00.250Z`, `...+02:00`) as epoch milliseconds.
pub fn rfc3339_ms(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let num = |i: usize, n: usize| -> Option<i64> {
        let mut v = 0i64;
        for k in i..i + n {
            let d = b.get(k)?.wrapping_sub(b'0');
            if d > 9 {
                return None;
            }
            v = v * 10 + d as i64;
        }
        Some(v)
    };
    let (y, mo, d, h, mi, se) = (num(0, 4)?, num(5, 2)?, num(8, 2)?, num(11, 2)?, num(14, 2)?, num(17, 2)?);
    let mut i = 19;
    let mut frac = 0.0;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let mut scale = 0.1;
        while let Some(c) = b.get(i).filter(|c| c.is_ascii_digit()) {
            frac += (c - b'0') as f64 * scale;
            scale /= 10.0;
            i += 1;
        }
    }
    let off = match *b.get(i)? {
        b'Z' | b'z' => 0,
        s @ (b'+' | b'-') => {
            let o = num(i + 1, 2)? * 60 + num(i + 4, 2)?;
            if s == b'-' {
                -o
            } else {
                o
            }
        }
        _ => return None,
    };
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se - off * 60;
    Some((secs as f64 + frac) * 1000.0)
}

/// One CSV field: empty is missing (NaN), a number, or an RFC 3339 time in epoch ms.
#[cfg(feature = "csv")]
fn field(f: &str) -> f64 {
    if f.is_empty() {
        return f64::NAN;
    }
    match f.parse::<f64>() {
        Ok(v) => v,
        Err(_) => rfc3339_ms(f).unwrap_or(f64::NAN),
    }
}

/// CSV with a header line, numeric or RFC 3339 fields, no quoting, into one column per header field.
#[cfg(feature = "csv")]
pub fn parse_csv(s: &str, cols: &mut Vec<Vec<f64>>) -> Result<usize, &'static str> {
    let mut lines = s.split('\n');
    let header = lines.next().ok_or("empty")?;
    let ncol = header.split(',').count();
    cols.clear();
    cols.resize_with(ncol, Vec::new);
    let mut rows = 0;
    for line in lines {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let mut it = line.split(',');
        for col in cols.iter_mut() {
            col.push(field(it.next().unwrap_or("")));
        }
        rows += 1;
    }
    Ok(rows)
}

// ---------------------------------------------------------------- JSON

/// `{"t":[...],"v":[...]}`: columns by name, null for missing.
#[cfg(feature = "json")]
pub fn parse_json_columns(s: &str, cols: &mut Vec<Vec<f64>>) -> Result<usize, String> {
    let m: std::collections::BTreeMap<String, Vec<Option<f64>>> = serde_json::from_str(s).map_err(|e| e.to_string())?;
    cols.clear();
    let mut rows = 0;
    for (_, c) in m {
        rows = c.len();
        cols.push(c.into_iter().map(|x| x.unwrap_or(f64::NAN)).collect());
    }
    Ok(rows)
}

/// A row object: `{"t": 1, "v": 2.5}`.
#[cfg(feature = "json")]
#[derive(serde::Deserialize)]
struct Row {
    t: f64,
    v: Option<f64>,
}

/// `[{"t":..,"v":..}, ...]`: rows of objects, as most APIs return them.
#[cfg(feature = "json")]
pub fn parse_json_rows(s: &str, cols: &mut Vec<Vec<f64>>) -> Result<usize, String> {
    let rows: Vec<Row> = serde_json::from_str(s).map_err(|e| e.to_string())?;
    cols.clear();
    cols.push(rows.iter().map(|r| r.t).collect());
    cols.push(rows.iter().map(|r| r.v.unwrap_or(f64::NAN)).collect());
    Ok(rows.len())
}

// ---------------------------------------------------------------- Arrow IPC, by hand

/// A minimal Arrow IPC stream reader: Schema then RecordBatch messages, columns
/// of Float64, Int64 and Timestamp (as f64; timestamps in ms), validity bitmaps
/// to NaN, no compression, no dictionaries. Flatbuffers are read by hand.
#[cfg(feature = "arrowmin")]
pub mod ipc {
    fn u16_at(b: &[u8], p: usize) -> Option<u16> {
        Some(u16::from_le_bytes(b.get(p..p + 2)?.try_into().ok()?))
    }
    fn u32_at(b: &[u8], p: usize) -> Option<u32> {
        Some(u32::from_le_bytes(b.get(p..p + 4)?.try_into().ok()?))
    }
    fn i64_at(b: &[u8], p: usize) -> Option<i64> {
        Some(i64::from_le_bytes(b.get(p..p + 8)?.try_into().ok()?))
    }
    /// Where field `i` of the table at `t` is stored, if present.
    fn field(b: &[u8], t: usize, i: usize) -> Option<usize> {
        let vt = (t as i64 - u32_at(b, t)? as i32 as i64) as usize;
        let size = u16_at(b, vt)? as usize;
        if 4 + 2 * i >= size {
            return None;
        }
        match u16_at(b, vt + 4 + 2 * i)? {
            0 => None,
            o => Some(t + o as usize),
        }
    }
    /// Follows the offset stored at `p` (a table, vector or string).
    fn deref(b: &[u8], p: usize) -> Option<usize> {
        Some(p + u32_at(b, p)? as usize)
    }
    fn table(b: &[u8], t: usize, i: usize) -> Option<usize> {
        deref(b, field(b, t, i)?)
    }
    fn byte(b: &[u8], t: usize, i: usize) -> u8 {
        field(b, t, i).and_then(|p| b.get(p).copied()).unwrap_or(0)
    }
    fn short(b: &[u8], t: usize, i: usize) -> u16 {
        field(b, t, i).and_then(|p| u16_at(b, p)).unwrap_or(0)
    }

    /// A column's type, as far as a chart cares.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Kind {
        F64,
        I64,
        /// Timestamp; the factor to milliseconds.
        Time(f64),
        Other,
    }

    /// One column: name, kind, values as f64.
    pub struct Column {
        pub name: String,
        pub kind: Kind,
        pub values: Vec<f64>,
    }

    /// Reads an IPC stream into columns.
    pub fn read(s: &[u8]) -> Result<Vec<Column>, &'static str> {
        let mut cols: Vec<Column> = Vec::new();
        let mut p = 0;
        loop {
            let mut len = u32_at(s, p).ok_or("truncated")?;
            p += 4;
            if len == 0xFFFF_FFFF {
                len = u32_at(s, p).ok_or("truncated")?;
                p += 4;
            }
            if len == 0 {
                break;
            }
            let meta = s.get(p..p + len as usize).ok_or("truncated")?;
            p += len as usize;
            let msg = deref(meta, 0).ok_or("bad message")?;
            let kind = byte(meta, msg, 1);
            let header = table(meta, msg, 2).ok_or("no header")?;
            let body_len = field(meta, msg, 3).and_then(|q| i64_at(meta, q)).unwrap_or(0) as usize;
            let body = s.get(p..p + body_len).ok_or("truncated body")?;
            p += body_len;
            match kind {
                1 => {
                    let fields = table(meta, header, 1).ok_or("no fields")?;
                    let n = u32_at(meta, fields).ok_or("bad")? as usize;
                    for k in 0..n {
                        let f = deref(meta, fields + 4 + 4 * k).ok_or("bad field")?;
                        let name = table(meta, f, 0)
                            .and_then(|q| {
                                let l = u32_at(meta, q)? as usize;
                                std::str::from_utf8(meta.get(q + 4..q + 4 + l)?).ok()
                            })
                            .unwrap_or("")
                            .to_string();
                        let tt = byte(meta, f, 2);
                        let ty = table(meta, f, 3);
                        let kind = match (tt, ty) {
                            (3, Some(ty)) if short(meta, ty, 0) == 2 => Kind::F64,
                            (2, Some(ty)) if field(meta, ty, 0).and_then(|q| u32_at(meta, q)) == Some(64) => Kind::I64,
                            (10, Some(ty)) => Kind::Time([1000.0, 1.0, 1e-3, 1e-6][short(meta, ty, 0).min(3) as usize]),
                            _ => Kind::Other,
                        };
                        cols.push(Column { name, kind, values: Vec::new() });
                    }
                }
                3 => {
                    let rows = field(meta, header, 0).and_then(|q| i64_at(meta, q)).unwrap_or(0) as usize;
                    let nodes = table(meta, header, 1).ok_or("no nodes")?;
                    let bufs = table(meta, header, 2).ok_or("no buffers")?;
                    if field(meta, header, 3).is_some() {
                        return Err("compressed");
                    }
                    for (c, col) in cols.iter_mut().enumerate() {
                        let nulls = i64_at(meta, nodes + 4 + 16 * c + 8).ok_or("bad node")?;
                        let buf = |k: usize| -> Option<&[u8]> {
                            let off = i64_at(meta, bufs + 4 + 16 * k)? as usize;
                            let len = i64_at(meta, bufs + 4 + 16 * k + 8)? as usize;
                            body.get(off..off + len)
                        };
                        let valid = buf(2 * c).ok_or("bad buffer")?;
                        let vals = buf(2 * c + 1).ok_or("bad buffer")?;
                        let start = col.values.len();
                        let chunks = vals.chunks_exact(8).take(rows);
                        match col.kind {
                            Kind::F64 => col.values.extend(chunks.map(|x| f64::from_le_bytes(x.try_into().unwrap()))),
                            Kind::I64 => col.values.extend(chunks.map(|x| i64::from_le_bytes(x.try_into().unwrap()) as f64)),
                            Kind::Time(k) => col.values.extend(chunks.map(|x| i64::from_le_bytes(x.try_into().unwrap()) as f64 * k)),
                            Kind::Other => col.values.resize(start + rows, f64::NAN),
                        }
                        if nulls > 0 && !valid.is_empty() {
                            for i in 0..rows {
                                if valid[i >> 3] & (1 << (i & 7)) == 0 {
                                    col.values[start + i] = f64::NAN;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(cols)
    }
}

// ---------------------------------------------------------------- Arrow IPC, arrow-rs

/// The same through arrow-rs (arrow-ipc's StreamReader).
#[cfg(feature = "arrowrs")]
pub fn read_arrow_rs(bytes: &[u8], cols: &mut Vec<Vec<f64>>) -> Result<usize, String> {
    use arrow_array::cast::AsArray;
    use arrow_array::types::{Float64Type, Int64Type, TimestampMillisecondType};
    use arrow_schema::{DataType, TimeUnit};
    let reader = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None).map_err(|e| e.to_string())?;
    let n = reader.schema().fields().len();
    cols.clear();
    cols.resize_with(n, Vec::new);
    let mut rows = 0;
    for batch in reader {
        let batch = batch.map_err(|e| e.to_string())?;
        rows += batch.num_rows();
        for (c, a) in batch.columns().iter().enumerate() {
            let out = &mut cols[c];
            match a.data_type() {
                DataType::Float64 => out.extend(a.as_primitive::<Float64Type>().iter().map(|x| x.unwrap_or(f64::NAN))),
                DataType::Int64 => out.extend(a.as_primitive::<Int64Type>().iter().map(|x| x.map_or(f64::NAN, |x| x as f64))),
                DataType::Timestamp(TimeUnit::Millisecond, _) => {
                    out.extend(a.as_primitive::<TimestampMillisecondType>().iter().map(|x| x.map_or(f64::NAN, |x| x as f64)))
                }
                _ => out.resize(out.len() + a.len(), f64::NAN),
            }
        }
    }
    Ok(rows)
}

// ---------------------------------------------------------------- ring buffer

/// A fixed-capacity ring of (t, v) samples; the oldest drop off when full.
#[cfg(feature = "ring")]
pub struct Ring {
    pub t: Vec<f64>,
    pub v: Vec<f64>,
    head: usize,
    len: usize,
}

#[cfg(feature = "ring")]
impl Ring {
    /// A ring holding at most `cap` samples, allocated once.
    pub fn new(cap: usize) -> Ring {
        Ring { t: vec![0.0; cap], v: vec![0.0; cap], head: 0, len: 0 }
    }
    /// Samples held.
    pub fn len(&self) -> usize {
        self.len
    }
    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Appends one sample.
    pub fn push(&mut self, t: f64, v: f64) {
        let cap = self.t.len();
        self.t[self.head] = t;
        self.v[self.head] = v;
        self.head += 1;
        if self.head == cap {
            self.head = 0;
        }
        self.len = (self.len + 1).min(cap);
    }
    /// Appends a batch (at most two copies); a batch longer than the ring keeps its tail.
    pub fn push_batch(&mut self, t: &[f64], v: &[f64]) {
        let cap = self.t.len();
        let n = t.len().min(v.len());
        let (t, v) = (&t[n - n.min(cap)..n], &v[n - n.min(cap)..n]);
        let n = t.len();
        let first = n.min(cap - self.head);
        self.t[self.head..self.head + first].copy_from_slice(&t[..first]);
        self.v[self.head..self.head + first].copy_from_slice(&v[..first]);
        self.t[..n - first].copy_from_slice(&t[first..]);
        self.v[..n - first].copy_from_slice(&v[first..]);
        self.head = (self.head + n) % cap;
        self.len = (self.len + n).min(cap);
    }
    /// The samples oldest first, as up to two contiguous segments of (t, v).
    pub fn segments(&self) -> [(&[f64], &[f64]); 2] {
        let cap = self.t.len();
        if self.len < cap {
            [(&self.t[..self.len], &self.v[..self.len]), (&[], &[])]
        } else {
            [(&self.t[self.head..], &self.v[self.head..]), (&self.t[..self.head], &self.v[..self.head])]
        }
    }
}

// ---------------------------------------------------------------- downsampling

/// Output of a downsampler: points in x order.
#[cfg(feature = "downsample")]
#[derive(Default)]
pub struct Points {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

/// Per-column aggregates: first, last, min and max, each with its index.
#[cfg(feature = "downsample")]
#[derive(Clone, Copy)]
struct Bucket {
    col: i64,
    first: (usize, f64, f64),
    last: (usize, f64, f64),
    min: (usize, f64, f64),
    max: (usize, f64, f64),
}

/// M4 (Jugel et al. 2014) when `m4` is true, else min-max per column: x
/// sorted ascending, the view [x0, x1) split into `width` dot columns; NaN
/// values are skipped. Data is given as segments (a ring has two).
#[cfg(feature = "downsample")]
pub fn columns(segs: &[(&[f64], &[f64])], x0: f64, x1: f64, width: usize, m4: bool, out: &mut Points) {
    out.x.clear();
    out.y.clear();
    let scale = width as f64 / (x1 - x0);
    let mut b: Option<Bucket> = None;
    let flush = |b: &Bucket, out: &mut Points| {
        let mut p = [b.first, b.min, b.max, b.last];
        let k = if m4 {
            p.sort_unstable_by_key(|q| q.0);
            4
        } else {
            p[0] = b.min;
            p[1] = b.max;
            p[..2].sort_unstable_by_key(|q| q.0);
            2
        };
        let mut prev = usize::MAX;
        for q in &p[..k] {
            if q.0 != prev {
                out.x.push(q.1);
                out.y.push(q.2);
                prev = q.0;
            }
        }
    };
    let mut base = 0;
    for (xs, ys) in segs {
        let lo = xs.partition_point(|&x| x < x0);
        let hi = xs.partition_point(|&x| x < x1);
        for i in lo..hi {
            let (x, y) = (xs[i], ys[i]);
            if y.is_nan() {
                continue;
            }
            let col = ((x - x0) * scale) as i64;
            let q = (base + i, x, y);
            match &mut b {
                Some(c) if c.col == col => {
                    c.last = q;
                    if y < c.min.2 {
                        c.min = q;
                    }
                    if y > c.max.2 {
                        c.max = q;
                    }
                }
                _ => {
                    if let Some(c) = &b {
                        flush(c, out);
                    }
                    b = Some(Bucket { col, first: q, last: q, min: q, max: q });
                }
            }
        }
        base += xs.len();
    }
    if let Some(c) = &b {
        flush(c, out);
    }
}

/// Largest-Triangle-Three-Buckets (Steinarsson 2013) to `threshold` points over the view [x0, x1).
#[cfg(feature = "downsample")]
pub fn lttb(segs: &[(&[f64], &[f64])], x0: f64, x1: f64, threshold: usize, out: &mut Points) {
    out.x.clear();
    out.y.clear();
    // The view as one indexable range over the segments.
    let mut parts: [(&[f64], &[f64]); 2] = [(&[], &[]), (&[], &[])];
    for (k, (xs, ys)) in segs.iter().take(2).enumerate() {
        let lo = xs.partition_point(|&x| x < x0);
        let hi = xs.partition_point(|&x| x < x1);
        parts[k] = (&xs[lo..hi], &ys[lo..hi]);
    }
    let n0 = parts[0].0.len();
    let n = n0 + parts[1].0.len();
    let at = |i: usize| if i < n0 { (parts[0].0[i], parts[0].1[i]) } else { (parts[1].0[i - n0], parts[1].1[i - n0]) };
    if threshold >= n || threshold < 3 {
        for i in 0..n {
            let (x, y) = at(i);
            out.x.push(x);
            out.y.push(y);
        }
        return;
    }
    let every = (n - 2) as f64 / (threshold - 2) as f64;
    let mut a = 0;
    let (ax0, ay0) = at(0);
    out.x.push(ax0);
    out.y.push(ay0);
    for i in 0..threshold - 2 {
        let s = ((i + 1) as f64 * every) as usize + 1;
        let e = (((i + 2) as f64 * every) as usize + 1).min(n);
        let (mut sx, mut sy) = (0.0, 0.0);
        for j in s..e {
            let (x, y) = at(j);
            sx += x;
            sy += y;
        }
        let m = (e - s).max(1) as f64;
        let (cx, cy) = (sx / m, sy / m);
        let rs = (i as f64 * every) as usize + 1;
        let re = ((i + 1) as f64 * every) as usize + 1;
        let (ax, ay) = at(a);
        let mut best = (-1.0, rs);
        for j in rs..re {
            let (x, y) = at(j);
            let area = ((ax - cx) * (y - ay) - (ax - x) * (cy - ay)).abs();
            if area > best.0 {
                best = (area, j);
            }
        }
        a = best.1;
        let (x, y) = at(a);
        out.x.push(x);
        out.y.push(y);
    }
    let (x, y) = at(n - 1);
    out.x.push(x);
    out.y.push(y);
}

// ---------------------------------------------------------------- planets

/// Heliocentric ecliptic J2000 position (au) of a planet (0 Mercury .. 7 Neptune,
/// 2 the Earth-Moon barycentre) at a Julian date, from JPL's approximate
/// Keplerian elements (ssd.jpl.nasa.gov/planets/approx_pos.html, Table 1, 1800-2050).
#[cfg(feature = "kepler")]
pub fn kepler(planet: usize, jd: f64) -> [f64; 3] {
    #[rustfmt::skip]
    const E: [[f64; 12]; 8] = [
        [0.38709927, 0.20563593, 7.00497902, 252.25032350, 77.45779628, 48.33076593, 0.00000037, 0.00001906, -0.00594749, 149472.67411175, 0.16047689, -0.12534081],
        [0.72333566, 0.00677672, 3.39467605, 181.97909950, 131.60246718, 76.67984255, 0.00000390, -0.00004107, -0.00078890, 58517.81538729, 0.00268329, -0.27769418],
        [1.00000261, 0.01671123, -0.00001531, 100.46457166, 102.93768193, 0.0, 0.00000562, -0.00004392, -0.01294668, 35999.37244981, 0.32327364, 0.0],
        [1.52371034, 0.09339410, 1.84969142, -4.55343205, -23.94362959, 49.55953891, 0.00001847, 0.00007882, -0.00813131, 19140.30268499, 0.44441088, -0.29257343],
        [5.20288700, 0.04838624, 1.30439695, 34.39644051, 14.72847983, 100.47390909, -0.00011607, -0.00013253, -0.00183714, 3034.74612775, 0.21252668, 0.20469106],
        [9.53667594, 0.05386179, 2.48599187, 49.95424423, 92.59887831, 113.66242448, -0.00125060, -0.00050991, 0.00193609, 1222.49362201, -0.41897216, -0.28867794],
        [19.18916464, 0.04725744, 0.77263783, 313.23810451, 170.95427630, 74.01692503, -0.00196176, -0.00004397, -0.00242939, 428.48202785, 0.40805281, 0.04240589],
        [30.06992276, 0.00859048, 1.77004347, -55.12002969, 44.96476227, 131.78422574, 0.00026291, 0.00005105, 0.00035372, 218.45945325, -0.32241464, -0.00508664],
    ];
    let e = &E[planet];
    let c = (jd - 2_451_545.0) / 36_525.0;
    let el = |k: usize| e[k] + e[k + 6] * c;
    let (a, ec, i, l, w, o) = (el(0), el(1), el(2).to_radians(), el(3), el(4), el(5));
    let om = w - o;
    let m = ((l - w) % 360.0 + 540.0) % 360.0 - 180.0;
    let m = m.to_radians();
    let mut ea = m + ec * m.sin();
    for _ in 0..6 {
        ea -= (ea - ec * ea.sin() - m) / (1.0 - ec * ea.cos());
    }
    let (xp, yp) = (a * (ea.cos() - ec), a * (1.0 - ec * ec).sqrt() * ea.sin());
    let (om, o) = (om.to_radians(), o.to_radians());
    let (co, so, cw, sw, ci, si) = (o.cos(), o.sin(), om.cos(), om.sin(), i.cos(), i.sin());
    [
        (cw * co - sw * so * ci) * xp + (-sw * co - cw * so * ci) * yp,
        (cw * so + sw * co * ci) * xp + (-sw * so + cw * co * ci) * yp,
        (sw * si) * xp + (cw * si) * yp,
    ]
}

/// The same from VSOP87A (the vsop87 crate): heliocentric ecliptic J2000, au.
#[cfg(feature = "vsop")]
pub fn vsop(planet: usize, jd: f64) -> [f64; 3] {
    use vsop87::vsop87a as v;
    let r = match planet {
        0 => v::mercury(jd),
        1 => v::venus(jd),
        2 => v::earth_moon(jd),
        3 => v::mars(jd),
        4 => v::jupiter(jd),
        5 => v::saturn(jd),
        6 => v::uranus(jd),
        _ => v::neptune(jd),
    };
    [r.x, r.y, r.z]
}

/// Geocentric right ascension and declination (degrees) of a heliocentric
/// ecliptic J2000 position seen from `earth`: the ecliptic-to-equatorial
/// rotation by the J2000 obliquity, 23.43928 degrees.
pub fn ra_dec(p: [f64; 3], earth: [f64; 3]) -> (f64, f64) {
    let (x, y, z) = (p[0] - earth[0], p[1] - earth[1], p[2] - earth[2]);
    let eps = 23.43928f64.to_radians();
    let (ye, ze) = (y * eps.cos() - z * eps.sin(), y * eps.sin() + z * eps.cos());
    let ra = ye.atan2(x).to_degrees().rem_euclid(360.0);
    let dec = ze.atan2((x * x + ye * ye).sqrt()).to_degrees();
    (ra, dec)
}
