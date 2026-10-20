//! Throwaway code for discovery/testing.md: seeded fixture datasets, a
//! min-max-per-column reducer, a port of LTTB's reference implementation and
//! a port of d3-array 3.2.4's `ticks`, so that fixture tests, property tests
//! and their costs can be measured. Not glyphplot code.

/// SplitMix64: a seeded, portable PRNG (the same stream on every target).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1), 53 bits.
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// One x and one y column.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

/// The fixture names, as the contract would loop over them.
pub const FIXTURES: [&str; 14] = [
    "empty",
    "one",
    "two",
    "sine-1k",
    "random-walk-100k",
    "random-walk-1m",
    "nan-gaps",
    "constant",
    "huge-range",
    "tiny-range",
    "negative",
    "unsorted-x",
    "duplicate-x",
    "steps-10k",
];

/// A random walk of `n` points, x = 0..n.
pub fn random_walk(n: usize, seed: u64) -> Series {
    let mut r = Rng::new(seed);
    let mut v = 100.0;
    let x = (0..n).map(|i| i as f64).collect();
    let y = (0..n)
        .map(|_| {
            v += r.f64() - 0.5;
            v
        })
        .collect();
    Series { x, y }
}

/// A fixture by name, generated from a seed: nothing checked in.
pub fn fixture(name: &str) -> Series {
    let seq = |n: usize| (0..n).map(|i| i as f64).collect::<Vec<_>>();
    match name {
        "empty" => Series { x: vec![], y: vec![] },
        "one" => Series { x: vec![0.0], y: vec![42.0] },
        "two" => Series { x: vec![0.0, 1.0], y: vec![1.0, 2.0] },
        "sine-1k" => Series { x: seq(1000), y: (0..1000).map(|i| (i as f64 / 50.0).sin()).collect() },
        "random-walk-100k" => random_walk(100_000, 1),
        "random-walk-1m" => random_walk(1_000_000, 2),
        "nan-gaps" => {
            let mut s = random_walk(10_000, 3);
            for i in (0..s.y.len()).filter(|i| (i / 500) % 4 == 3) {
                s.y[i] = f64::NAN;
            }
            s
        }
        "constant" => Series { x: seq(1000), y: vec![7.0; 1000] },
        "huge-range" => {
            let mut r = Rng::new(4);
            Series { x: seq(1000), y: (0..1000).map(|_| 10f64.powf(r.f64() * 600.0 - 300.0)).collect() }
        }
        "tiny-range" => Series { x: seq(1000), y: (0..1000).map(|i| 1.0 + (i as f64) * 1e-15).collect() },
        "negative" => {
            let mut s = random_walk(1000, 5);
            s.y.iter_mut().for_each(|v| *v -= 150.0);
            s
        }
        "unsorted-x" => {
            let mut s = random_walk(1000, 6);
            let mut r = Rng::new(7);
            for i in (1..s.x.len()).rev() {
                let j = (r.next_u64() % (i as u64 + 1)) as usize;
                s.x.swap(i, j);
                s.y.swap(i, j);
            }
            s
        }
        "duplicate-x" => Series { x: (0..1000).map(|i| (i / 3) as f64).collect(), y: random_walk(1000, 8).y },
        "steps-10k" => Series { x: seq(10_000), y: (0..10_000).map(|i| ((i / 1000) % 3) as f64).collect() },
        _ => panic!("no fixture {name}"),
    }
}

/// The column an x falls in: one function for the reducer and for a hit
/// test, so the two can never disagree by a rounding.
pub fn column_of(x: f64, x0: f64, x1: f64, cols: usize) -> usize {
    (((x - x0) * (cols as f64 / (x1 - x0))) as usize).min(cols - 1)
}

/// Min and max of y per column over x in [x0, x1): what a braille line needs
/// a column at a time. NaN is skipped; an empty column is (NaN, NaN).
pub fn minmax(s: &Series, x0: f64, x1: f64, cols: usize) -> Vec<(f64, f64)> {
    let mut out = vec![(f64::NAN, f64::NAN); cols];
    for (&x, &y) in s.x.iter().zip(&s.y) {
        if y.is_nan() || !(x >= x0 && x < x1) {
            continue;
        }
        let c = column_of(x, x0, x1, cols);
        let (lo, hi) = &mut out[c];
        if lo.is_nan() || y < *lo {
            *lo = y;
        }
        if hi.is_nan() || y > *hi {
            *hi = y;
        }
    }
    out
}

/// Largest-Triangle-Three-Buckets: a line-for-line port of
/// sveinn-steinarsson/flot-downsample (MIT) at af872e0, returning the indices
/// kept.
pub fn lttb(x: &[f64], y: &[f64], threshold: usize) -> Vec<usize> {
    let n = x.len();
    if threshold >= n || threshold == 0 {
        return (0..n).collect();
    }
    let every = (n - 2) as f64 / (threshold - 2) as f64;
    let mut out = Vec::with_capacity(threshold);
    let mut a = 0;
    out.push(a);
    for i in 0..threshold - 2 {
        let mut s = ((i + 1) as f64 * every).floor() as usize + 1;
        let e = (((i + 2) as f64 * every).floor() as usize + 1).min(n);
        let len = (e - s) as f64;
        let (mut ax, mut ay) = (0.0, 0.0);
        while s < e {
            ax += x[s];
            ay += y[s];
            s += 1;
        }
        ax /= len;
        ay /= len;
        let from = (i as f64 * every).floor() as usize + 1;
        let to = ((i + 1) as f64 * every).floor() as usize + 1;
        let (px, py) = (x[a], y[a]);
        let mut max = -1.0;
        let mut next = a;
        for b in from..to {
            let area = ((px - ax) * (y[b] - py) - (px - x[b]) * (ay - py)).abs() * 0.5;
            if area > max {
                max = area;
                next = b;
            }
        }
        out.push(next);
        a = next;
    }
    out.push(n - 1);
    out
}

/// JavaScript's Math.round: the nearest integer, ties toward +infinity
/// (Rust's `round` sends ties away from zero, so -2.5 gives -3, not -2).
pub fn js_round(v: f64) -> f64 {
    if cfg!(feature = "naive-round") {
        return v.round();
    }
    let f = v.floor();
    if v - f >= 0.5 { f + 1.0 } else { f }
}

/// Which log10 and pow the ticks port uses.
pub fn log10(v: f64) -> f64 {
    v.log10()
}

const E10: f64 = 7.0710678118654755; // Math.sqrt(50)
const E5: f64 = 3.1622776601683795; // Math.sqrt(10)
const E2: f64 = 1.4142135623730951; // Math.sqrt(2)

fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let step = (stop - start) / count.max(0.0);
    let power = log10(step).floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= E10 {
        10.0
    } else if error >= E5 {
        5.0
    } else if error >= E2 {
        2.0
    } else {
        1.0
    };
    let (mut i1, mut i2, mut inc);
    if power < 0.0 {
        inc = 10f64.powf(-power) / factor;
        i1 = js_round(start * inc);
        i2 = js_round(stop * inc);
        if i1 / inc < start {
            i1 += 1.0;
        }
        if i2 / inc > stop {
            i2 -= 1.0;
        }
        inc = -inc;
    } else {
        inc = 10f64.powf(power) * factor;
        i1 = js_round(start / inc);
        i2 = js_round(stop / inc);
        if i1 * inc < start {
            i1 += 1.0;
        }
        if i2 * inc > stop {
            i2 -= 1.0;
        }
    }
    if i2 < i1 && 0.5 <= count && count < 2.0 {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

/// d3-array 3.2.4's `ticks(start, stop, count)`, ported line for line.
pub fn ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if !(count > 0.0) {
        return vec![];
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (i1, i2, inc) = if reverse { tick_spec(stop, start, count) } else { tick_spec(start, stop, count) };
    if !(i2 >= i1) {
        return vec![];
    }
    let n = (i2 - i1 + 1.0) as usize;
    (0..n)
        .map(|i| {
            let i = i as f64;
            match (reverse, inc < 0.0) {
                (true, true) => (i2 - i) / -inc,
                (true, false) => (i2 - i) * inc,
                (false, true) => (i1 + i) / -inc,
                (false, false) => (i1 + i) * inc,
            }
        })
        .collect()
}

/// For the Node check of the wasm build: room for `n` f64s.
#[unsafe(no_mangle)]
pub extern "C" fn alloc_f64(n: usize) -> *mut f64 {
    let mut v = vec![0.0f64; n];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// For the Node check: `ticks` into `out` (room for `cap`), returning how many.
///
/// # Safety
/// `out` must hold `cap` f64s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ticks_into(start: f64, stop: f64, count: f64, out: *mut f64, cap: usize) -> usize {
    let t = ticks(start, stop, count);
    let n = t.len().min(cap);
    // SAFETY: the caller gives room for `cap`.
    unsafe { std::ptr::copy_nonoverlapping(t.as_ptr(), out, n) };
    t.len()
}

/// For the Node check: LTTB's kept indices into `out` (as f64), returning how many.
///
/// # Safety
/// `x` and `y` must hold `n` f64s, `out` `threshold`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lttb_into(x: *const f64, y: *const f64, n: usize, threshold: usize, out: *mut f64) -> usize {
    // SAFETY: the caller gives `n` of each.
    let (x, y) = unsafe { (std::slice::from_raw_parts(x, n), std::slice::from_raw_parts(y, n)) };
    let k = lttb(x, y, threshold);
    for (i, &v) in k.iter().enumerate() {
        // SAFETY: room for `threshold`.
        unsafe { *out.add(i) = v as f64 };
    }
    k.len()
}
