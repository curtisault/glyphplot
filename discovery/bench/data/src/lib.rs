//! Throwaway bench for discovery/data.md: ways to get columns of numbers into
//! wasm, a ring buffer, downsamplers and planet positions, each behind its own
//! feature so a build with one more feature measures what it adds.

pub mod core;

use wasm_bindgen::prelude::*;

/// Columns of f64 held in wasm memory.
#[wasm_bindgen]
#[derive(Default)]
pub struct Table {
    cols: Vec<Vec<f64>>,
    #[cfg(feature = "arrowmin")]
    bytes: Vec<u8>,
    #[cfg(feature = "downsample")]
    out: core::Points,
}

#[wasm_bindgen]
impl Table {
    /// An empty table.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Table {
        Table::default()
    }

    fn col(&mut self, i: usize) -> &mut Vec<f64> {
        if self.cols.len() <= i {
            self.cols.resize_with(i + 1, Vec::new);
        }
        &mut self.cols[i]
    }

    /// (a) A Float64Array as `&[f64]`: the glue copies it into wasm memory, Rust copies it again into the column.
    pub fn set_slice(&mut self, i: usize, data: &[f64]) {
        let c = self.col(i);
        c.clear();
        c.extend_from_slice(data);
    }

    /// (a') A Float64Array as `Vec<f64>`: the glue copies it into wasm memory once and Rust takes it.
    pub fn set_vec(&mut self, i: usize, data: Vec<f64>) {
        *self.col(i) = data;
    }

    /// (b) A wasm-owned column of `n` values for JS to write into: returns its address.
    pub fn reserve(&mut self, i: usize, n: usize) -> *mut f64 {
        let c = self.col(i);
        c.clear();
        c.resize(n, 0.0);
        c.as_mut_ptr()
    }

    /// Rows in column `i`.
    pub fn len(&self, i: usize) -> usize {
        self.cols.get(i).map_or(0, Vec::len)
    }

    /// Whether the table has no columns.
    pub fn is_empty(&self) -> bool {
        self.cols.is_empty()
    }

    /// Sum of column `i`, so a test can check what arrived.
    pub fn sum(&self, i: usize) -> f64 {
        self.cols.get(i).map_or(0.0, |c| c.iter().filter(|x| !x.is_nan()).sum())
    }

    /// (c) JSON columns `{"t":[...],"v":[...]}`.
    #[cfg(feature = "json")]
    pub fn set_json_columns(&mut self, s: &str) -> Result<usize, JsError> {
        core::parse_json_columns(s, &mut self.cols).map_err(|e| JsError::new(&e))
    }

    /// (c') JSON rows `[{"t":..,"v":..}]`.
    #[cfg(feature = "json")]
    pub fn set_json_rows(&mut self, s: &str) -> Result<usize, JsError> {
        core::parse_json_rows(s, &mut self.cols).map_err(|e| JsError::new(&e))
    }

    /// (d) CSV text with a header line.
    #[cfg(feature = "csv")]
    pub fn set_csv(&mut self, s: &str) -> Result<usize, JsError> {
        core::parse_csv(s, &mut self.cols).map_err(JsError::new)
    }

    /// (e) An Arrow IPC stream passed as `&[u8]` (copied in), read by hand.
    #[cfg(feature = "arrowmin")]
    pub fn set_arrow(&mut self, bytes: &[u8]) -> Result<usize, JsError> {
        let cols = core::ipc::read(bytes).map_err(JsError::new)?;
        let rows = cols.first().map_or(0, |c| c.values.len());
        self.cols = cols.into_iter().map(|c| c.values).collect();
        Ok(rows)
    }

    /// (e') A wasm-owned byte buffer for JS to write an IPC stream into: returns its address.
    #[cfg(feature = "arrowmin")]
    pub fn reserve_bytes(&mut self, n: usize) -> *mut u8 {
        self.bytes.clear();
        self.bytes.resize(n, 0);
        self.bytes.as_mut_ptr()
    }

    /// Reads the IPC stream JS wrote through `reserve_bytes`.
    #[cfg(feature = "arrowmin")]
    pub fn commit_arrow(&mut self) -> Result<usize, JsError> {
        let cols = core::ipc::read(&self.bytes).map_err(JsError::new)?;
        let rows = cols.first().map_or(0, |c| c.values.len());
        self.cols = cols.into_iter().map(|c| c.values).collect();
        Ok(rows)
    }

    /// (e'') The same stream through arrow-rs.
    #[cfg(feature = "arrowrs")]
    pub fn set_arrow_rs(&mut self, bytes: &[u8]) -> Result<usize, JsError> {
        core::read_arrow_rs(bytes, &mut self.cols).map_err(|e| JsError::new(&e))
    }

    /// Downsamples column 1 against column 0 over [x0, x1) to `width` dot
    /// columns: 0 min-max, 1 M4, 2 LTTB (to `width` points). Returns points out.
    #[cfg(feature = "downsample")]
    pub fn downsample(&mut self, how: u8, x0: f64, x1: f64, width: usize) -> usize {
        let segs = [(&self.cols[0][..], &self.cols[1][..])];
        match how {
            2 => core::lttb(&segs, x0, x1, width, &mut self.out),
            m => core::columns(&segs, x0, x1, width, m == 1, &mut self.out),
        }
        self.out.x.len()
    }
}

/// A ring of (t, v) samples for a live feed.
#[cfg(feature = "ring")]
#[wasm_bindgen]
pub struct Ring {
    ring: core::Ring,
    staging: Vec<f64>,
    #[cfg(feature = "downsample")]
    out: core::Points,
}

#[cfg(feature = "ring")]
#[wasm_bindgen]
impl Ring {
    /// A ring of `cap` samples, allocated once.
    #[wasm_bindgen(constructor)]
    pub fn new(cap: usize) -> Ring {
        Ring {
            ring: core::Ring::new(cap),
            staging: Vec::new(),
            #[cfg(feature = "downsample")]
            out: core::Points::default(),
        }
    }
    /// Samples held.
    pub fn len(&self) -> usize {
        self.ring.len()
    }
    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
    /// Appends one sample.
    pub fn push(&mut self, t: f64, v: f64) {
        self.ring.push(t, v);
    }
    /// Appends a batch passed as two Float64Arrays (copied in by the glue).
    pub fn push_batch(&mut self, t: &[f64], v: &[f64]) {
        self.ring.push_batch(t, v);
    }
    /// A staging area of `2n` values for JS to write a batch into (t then v): returns its address.
    pub fn staging(&mut self, n: usize) -> *mut f64 {
        self.staging.resize(2 * n, 0.0);
        self.staging.as_mut_ptr()
    }
    /// Appends the `n` samples JS wrote to the staging area.
    pub fn commit(&mut self, n: usize) {
        let (t, v) = self.staging.split_at(n);
        self.ring.push_batch(t, &v[..n]);
    }
    /// Downsamples the ring over [x0, x1): 0 min-max, 1 M4, 2 LTTB.
    #[cfg(feature = "downsample")]
    pub fn downsample(&mut self, how: u8, x0: f64, x1: f64, width: usize) -> usize {
        let segs = self.ring.segments();
        match how {
            2 => core::lttb(&segs, x0, x1, width, &mut self.out),
            m => core::columns(&segs, x0, x1, width, m == 1, &mut self.out),
        }
        self.out.x.len()
    }
}

/// RA and Dec of the eight planets at a Julian date from JPL's Keplerian elements, as 16 numbers (Earth's slot 0, 0).
#[cfg(feature = "kepler")]
#[wasm_bindgen]
pub fn planets_kepler(jd: f64) -> Vec<f64> {
    let earth = core::kepler(2, jd);
    (0..8).flat_map(|p| if p == 2 { [0.0, 0.0] } else { let (r, d) = core::ra_dec(core::kepler(p, jd), earth); [r, d] }).collect()
}

/// The same from VSOP87A.
#[cfg(feature = "vsop")]
#[wasm_bindgen]
pub fn planets_vsop(jd: f64) -> Vec<f64> {
    let earth = core::vsop(2, jd);
    (0..8).flat_map(|p| if p == 2 { [0.0, 0.0] } else { let (r, d) = core::ra_dec(core::vsop(p, jd), earth); [r, d] }).collect()
}

/// Reads an Arrow IPC stream with arrow2 and returns the rows of its first batch's columns, for arrow2's size only.
#[cfg(feature = "arrow2")]
#[wasm_bindgen]
pub fn arrow2_rows(bytes: &[u8]) -> Result<f64, JsError> {
    use arrow2::array::PrimitiveArray;
    use arrow2::io::ipc::read::{read_stream_metadata, StreamReader, StreamState};
    let mut r = std::io::Cursor::new(bytes);
    let meta = read_stream_metadata(&mut r).map_err(|e| JsError::new(&e.to_string()))?;
    let mut sum = 0.0;
    for s in StreamReader::new(r, meta, None) {
        if let StreamState::Some(chunk) = s.map_err(|e| JsError::new(&e.to_string()))? {
            for a in chunk.arrays() {
                if let Some(p) = a.as_any().downcast_ref::<PrimitiveArray<f64>>() {
                    sum += p.iter().flatten().sum::<f64>();
                }
            }
        }
    }
    Ok(sum)
}

/// Options as unicel reads them (a serde_json::Value object): how many keys. For the size of the base only.
#[cfg(feature = "options")]
#[wasm_bindgen]
pub fn options_keys(json: &str) -> usize {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(serde_json::Value::Object(m)) => m.len(),
        _ => 0,
    }
}
