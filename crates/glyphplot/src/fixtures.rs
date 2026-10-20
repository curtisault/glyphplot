//! The fixture datasets, generated from SplitMix64 with a fixed seed each
//! (REQ-FIX-010, ADR-128).
//!
//! [`FIXTURES`] lists the 23 fixtures of the plan's table
//! (`docs/IMPLEMENTATION_PLAN.md`, "Fixtures"); [`Fixture::generate`] makes
//! one on demand, so the million-row fixtures never live at once.
//!
//! Two generations are bitwise equal on every target, native and wasm32: the
//! generators use only operations IEEE 754 rounds exactly (`+`, `-`, `*`,
//! `/`, integer to and from float, comparisons), Rust's correctly rounded
//! decimal parsing, and constants. They call none of std's `sin`, `exp`, `ln`
//! or `powf`, whose results differ between platforms; [`sin_turns`] is the
//! sine they use. Every NaN is the constant `f64::NAN`, never the result of
//! arithmetic, whose bits wasm leaves unspecified. [`digest`] pins each
//! fixture's bits.

use crate::data::{Column, ColumnType, Data, Encode, Matrix, Role, Shape, Table, Values};

/// 2026-10-18T00:00:00Z in epoch milliseconds, where the time fixtures start.
pub const T0_MS: f64 = 1_792_281_600_000.0;

const DAY_MS: f64 = 86_400_000.0;

/// SplitMix64 (Steele, Lea and Flood, "Fast splittable pseudorandom number
/// generators", 2014): the same stream on every target.
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// A generator starting from `seed`.
    pub const fn new(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A float in [0, 1): the top 53 bits times 2^-53, exact.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    /// An integer in [0, `n`) by remainder (`n` > 0; the bias is immaterial here).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

/// The sine of `t` turns (`sin(2π t)`), from range reduction and a fixed
/// Taylor polynomial in IEEE-exact operations, so every target gives the same
/// bits. Within 1e-15 of std's `sin` for |t| ≤ 1 (the tests check 40,001
/// points), and periodic bit for bit. NaN for a value that is not finite.
pub fn sin_turns(t: f64) -> f64 {
    if !t.is_finite() {
        return f64::NAN;
    }
    // r = t - floor(t), in [0, 1): exact, as the integer part is removed.
    let whole = t as i64 as f64;
    let floor = if whole > t { whole - 1.0 } else { whole };
    let mut r = t - floor;
    if r >= 0.5 {
        r -= 1.0;
    }
    // sin(π - θ) = sin θ folds [-1/2, 1/2) onto [-1/4, 1/4], exactly (Sterbenz).
    if r > 0.25 {
        r = 0.5 - r;
    } else if r < -0.25 {
        r = -0.5 - r;
    }
    // The peaks exactly, which the polynomial misses by one ULP.
    if r == 0.25 || r == -0.25 {
        return r * 4.0;
    }
    let x = r * std::f64::consts::TAU;
    let x2 = x * x;
    let (mut sum, mut term) = (x, x);
    // Terms to x^23 / 23!; at |x| = π/2 the next is under 1e-19.
    for k in 1..=11u32 {
        term = term * -x2 / ((2 * k) * (2 * k + 1)) as f64;
        sum += term;
    }
    sum
}

/// One fixture of the plan's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fixture {
    /// Its name, as the plan's table gives it.
    pub name: &'static str,
    /// The data shape it is.
    pub shape: Shape,
    /// Its series: `y` columns of a wide table, or labels of a long table's `series` column.
    pub series: usize,
    /// Rows a series, as the table says; a matrix's rows (and its cols, which equal them).
    pub rows: usize,
    /// The SplitMix64 seed.
    pub seed: u64,
}

const fn fx(name: &'static str, shape: Shape, series: usize, rows: usize, k: u64) -> Fixture {
    // "glyplot" and the fixture's number: a fixed, distinct seed each.
    Fixture { name, shape, series, rows, seed: 0x676C_7970_6C6F_7400 + k }
}

/// The 23 fixtures of the plan's table, in its order (REQ-FIX-010).
pub const FIXTURES: [Fixture; 23] = [
    fx("empty", Shape::Series, 1, 0, 1),
    fx("one", Shape::Series, 1, 1, 2),
    fx("two", Shape::Series, 1, 2, 3),
    fx("sine-1k", Shape::Series, 1, 1_000, 4),
    fx("random-walk-100k", Shape::Series, 1, 100_000, 5),
    fx("random-walk-1m", Shape::Series, 1, 1_000_000, 6),
    fx("nan-gaps", Shape::Series, 1, 10_000, 7),
    fx("inf", Shape::Series, 1, 1_000, 8),
    fx("constant", Shape::Series, 1, 1_000, 9),
    fx("huge-range", Shape::Series, 1, 1_000, 10),
    fx("tiny-range", Shape::Series, 1, 1_000, 11),
    fx("negative", Shape::Series, 1, 1_000, 12),
    fx("zero-crossing", Shape::Series, 1, 1_000, 13),
    fx("unsorted-x", Shape::Series, 1, 1_000, 14),
    fx("duplicate-x", Shape::Series, 1, 1_000, 15),
    fx("steps-10k", Shape::Series, 1, 10_000, 16),
    fx("multi-4", Shape::Series, 4, 1_000, 17),
    fx("labels-wide", Shape::Series, 3, 100, 18),
    fx("ohlcv-days", Shape::Ohlcv, 1, 500, 19),
    fx("ohlcv-ticks-1m", Shape::Ohlcv, 1, 1_000_000, 20),
    fx("categorical-12", Shape::Counts, 1, 12, 21),
    fx("matrix-64", Shape::Matrix, 1, 64, 22),
    fx("timestamps-dst", Shape::Series, 1, 2_000, 23),
];

/// The fixture named `name`.
pub fn fixture(name: &str) -> Option<&'static Fixture> {
    FIXTURES.iter().find(|f| f.name == name)
}

/// The fixture named `name`, generated.
pub fn generate(name: &str) -> Option<Data> {
    fixture(name).map(Fixture::generate)
}

impl Fixture {
    /// The fixture's data, generated afresh from its seed. A series is a
    /// table `x` (role x) and `y` (role y); `multi-4` has four `y` columns;
    /// `labels-wide` is long, its names in a `series` category column,
    /// because column names are ASCII (data.schema.json).
    pub fn generate(&self) -> Data {
        let mut r = SplitMix64::new(self.seed);
        let n = self.rows;
        let index = || (0..n).map(|i| i as f64).collect::<Vec<f64>>();
        match self.name {
            "empty" | "one" | "two" => series(index(), (0..n).map(|_| r.next_f64() * 100.0).collect()),
            "sine-1k" => series(index(), (0..n).map(|i| sin_turns(i as f64 / 250.0)).collect()),
            "random-walk-100k" | "random-walk-1m" => {
                let t = (0..n).map(|i| T0_MS + i as f64 * 1000.0).collect();
                Data::from_table(table(vec![Column::timestamps_ms("t", t).role(Role::X), Column::f64("y", walk(&mut r, n, 100.0, 1.0)).role(Role::Y)]))
            }
            "nan-gaps" => {
                let mut y = walk(&mut r, n, 0.0, 1.0);
                y.iter_mut().enumerate().filter(|(i, _)| (i / 500) % 4 == 3).for_each(|(_, v)| *v = f64::NAN);
                series(index(), y)
            }
            "inf" => {
                let mut y = walk(&mut r, n, 0.0, 1.0);
                for v in &mut y {
                    match r.below(50) {
                        0 => *v = f64::INFINITY,
                        1 => *v = f64::NEG_INFINITY,
                        _ => {}
                    }
                }
                series(index(), y)
            }
            "constant" => series(index(), vec![42.0; n]),
            "huge-range" => series(index(), huge_range(&mut r, n)),
            "tiny-range" => series(index(), (0..n).map(|i| 1.0 + i as f64 * 1e-15).collect()),
            "negative" => series(index(), (0..n).map(|i| if i % 100 == 0 { 0.0 } else { -(1.0 + 99.0 * r.next_f64()) }).collect()),
            "zero-crossing" => series(index(), (0..n).map(|i| 10.0 * sin_turns(i as f64 / 200.0) + (r.next_f64() - 0.5)).collect()),
            "unsorted-x" => {
                let (mut x, mut y) = (index(), walk(&mut r, n, 0.0, 1.0));
                for i in (1..n).rev() {
                    let j = r.below(i as u64 + 1) as usize;
                    x.swap(i, j);
                    y.swap(i, j);
                }
                series(x, y)
            }
            "duplicate-x" => series((0..n).map(|i| (i / 3) as f64).collect(), (0..n).map(|_| r.next_f64() * 10.0).collect()),
            "steps-10k" => {
                let (mut level, mut left) = (0.0, 0u64);
                let y = (0..n)
                    .map(|_| {
                        if left == 0 {
                            level = r.below(100) as f64;
                            left = 1 + r.below(200);
                        }
                        left -= 1;
                        level
                    })
                    .collect();
                series(index(), y)
            }
            "multi-4" => {
                let mut cols = vec![Column::f64("x", index()).role(Role::X)];
                for (s, name) in ["alpha", "beta", "gamma", "delta"].into_iter().enumerate() {
                    cols.push(Column::f64(name, walk(&mut r, n, s as f64 * 2.0, 1.0)).role(Role::Y));
                }
                Data::from_table(table(cols))
            }
            "labels-wide" => labels_wide(&mut r, self.series, n),
            "ohlcv-days" => ohlcv_days(&mut r, n),
            "ohlcv-ticks-1m" => ohlcv_ticks(&mut r, n),
            "categorical-12" => categorical(&mut r),
            "matrix-64" => {
                let values = (0..n * n).map(|k| sin_turns((k / n) as f64 / 32.0) * sin_turns((k % n) as f64 / 16.0) + 0.1 * (r.next_f64() - 0.5)).collect();
                let labels = |p: &str| (0..n).map(|i| format!("{p}{i:02}")).collect();
                let m = Matrix::new(n, n, values).and_then(|m| m.labelled(labels("r"), labels("c"))).expect("a 64 × 64 matrix");
                let mut d = Data::new();
                d.push_matrix("m", m).expect("a fresh name");
                d
            }
            "timestamps-dst" => {
                // Hourly from 2026-10-01T00:00Z; New York leaves daylight time at
                // 2026-11-01T06:00Z, row 750.
                let start = T0_MS - 17.0 * DAY_MS;
                let t = (0..n).map(|i| start + i as f64 * 3_600_000.0).collect();
                let y = (0..n).map(|i| 15.0 + 8.0 * sin_turns(i as f64 / 24.0) + (r.next_f64() - 0.5)).collect();
                Data::from_table(table(vec![
                    Column::timestamps_ms("t", t).role(Role::X).zone("America/New_York"),
                    Column::f64("y", y).role(Role::Y).unit("°C"),
                ]))
            }
            other => unreachable!("fixture {other} has no generator"),
        }
    }
}

/// A table the generators build, which is valid by construction.
fn table(columns: Vec<Column>) -> Table {
    Table::from_columns(columns).expect("a fixture's columns are valid")
}

/// A series of `x` (role x) and `y` (role y).
fn series(x: Vec<f64>, y: Vec<f64>) -> Data {
    Data::from_table(table(vec![Column::f64("x", x).role(Role::X), Column::f64("y", y).role(Role::Y)]))
}

/// `n` steps of a random walk from `start`, each uniform in ±`step`/2.
fn walk(r: &mut SplitMix64, n: usize, start: f64, step: f64) -> Vec<f64> {
    let mut v = start;
    (0..n)
        .map(|_| {
            v += (r.next_f64() - 0.5) * step;
            v
        })
        .collect()
}

/// 10^-300 to 10^300: decimal exponents rising evenly and a random mantissa,
/// read by Rust's correctly rounded parser; the ends are exactly 1e-300 and 1e300.
fn huge_range(r: &mut SplitMix64, n: usize) -> Vec<f64> {
    let last = n.saturating_sub(1).max(1);
    (0..n)
        .map(|i| {
            let e = -300 + (600 * i / last) as i64;
            let m = r.below(9000);
            let text = if i == 0 || i == last { format!("1e{e}") } else { format!("{}.{:03}e{e}", 1 + m / 1000, m % 1000) };
            text.parse::<f64>().expect("a decimal literal")
        })
        .collect()
}

/// Three series of `n` rows in long form, interleaved so x is in order, named
/// in CJK and with combining marks.
fn labels_wide(r: &mut SplitMix64, s: usize, n: usize) -> Data {
    let names = vec![
        "東京都の平均気温 (℃)".to_string(),
        "Cafe\u{301} cre\u{300}me bru\u{302}le\u{301}e — Zu\u{308}rich".to_string(),
        "北京 Be\u{30C}ij\u{12B}ng 서울 데이터".to_string(),
    ];
    let mut levels: Vec<f64> = (0..s).map(|k| k as f64 * 5.0).collect();
    let (mut x, mut y, mut codes) = (Vec::with_capacity(n * s), Vec::with_capacity(n * s), Vec::with_capacity(n * s));
    for i in 0..n {
        for (k, level) in levels.iter_mut().enumerate() {
            *level += r.next_f64() - 0.5;
            x.push(i as f64);
            y.push(*level);
            codes.push(k as u32);
        }
    }
    Data::from_table(table(vec![
        Column::f64("x", x).role(Role::X),
        Column::f64("y", y).role(Role::Y),
        Column::categories("series", names, codes).role(Role::Series),
    ]))
}

/// `a` or `b`, whichever is larger, by comparison.
fn larger(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

/// `a` or `b`, whichever is smaller, by comparison.
fn smaller(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else {
        b
    }
}

/// The OHLCV table of columns `t`, `open`, `high`, `low`, `close`, `volume`.
fn ohlcv(t: Vec<f64>, o: Vec<f64>, h: Vec<f64>, l: Vec<f64>, c: Vec<f64>, v: Vec<f64>) -> Data {
    Data::from_table(table(vec![
        Column::timestamps_ms("t", t).role(Role::X),
        Column::f64("open", o).role(Role::Open).unit("USD"),
        Column::f64("high", h).role(Role::High).unit("USD"),
        Column::f64("low", l).role(Role::Low).unit("USD"),
        Column::f64("close", c).role(Role::Close).unit("USD"),
        Column::numbers("volume", ColumnType::I64, v).role(Role::Volume),
    ]))
}

/// The row of `ohlcv-days` that falls 10% and closes at its low.
pub const LIMIT_DOWN_ROW: usize = 250;

/// The row of `ohlcv-days` with no trades: zero volume and a flat bar.
pub const ZERO_VOLUME_ROW: usize = 333;

/// Daily bars on weekdays from Monday 2024-10-21 (no Saturdays or Sundays),
/// with a limit-down day and a day of zero volume.
fn ohlcv_days(r: &mut SplitMix64, n: usize) -> Data {
    // Epoch day 20017 is Monday 2024-10-21: (20017 + 4) % 7 == 1.
    let mut day = 20_017u64;
    let (mut t, mut o, mut h, mut l, mut c, mut v) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut last = 100.0;
    for i in 0..n {
        // 0 is Sunday and 6 Saturday.
        while matches!((day + 4) % 7, 0 | 6) {
            day += 1;
        }
        t.push(day as f64 * DAY_MS);
        day += 1;
        let open = last * (1.0 + 0.01 * (r.next_f64() - 0.5));
        let (bar, volume) = if i == LIMIT_DOWN_ROW {
            let close = open * 0.9;
            ((open, open, close, close), 5_000_000.0)
        } else if i == ZERO_VOLUME_ROW {
            ((last, last, last, last), 0.0)
        } else {
            let close = open * (1.0 + 0.04 * (r.next_f64() - 0.5));
            let high = larger(open, close) * (1.0 + 0.01 * r.next_f64());
            let low = smaller(open, close) * (1.0 - 0.01 * r.next_f64());
            ((open, high, low, close), (100_000 + r.below(900_000)) as f64)
        };
        o.push(bar.0);
        h.push(bar.1);
        l.push(bar.2);
        c.push(bar.3);
        v.push(volume);
        last = bar.3;
    }
    ohlcv(t, o, h, l, c, v)
}

/// One-second bars from [`T0_MS`], for resampling into candles.
fn ohlcv_ticks(r: &mut SplitMix64, n: usize) -> Data {
    let (mut o, mut h, mut l, mut c, mut v) =
        (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    let mut last = 100.0;
    for _ in 0..n {
        let close = last + (r.next_f64() - 0.5) * 0.1;
        o.push(last);
        h.push(larger(last, close) + r.next_f64() * 0.05);
        l.push(smaller(last, close) - r.next_f64() * 0.05);
        c.push(close);
        v.push(r.below(100) as f64);
        last = close;
    }
    ohlcv((0..n).map(|i| T0_MS + i as f64 * 1000.0).collect(), o, h, l, c, v)
}

/// Twelve categories with labels longer than a bar's band, and a count each.
fn categorical(r: &mut SplitMix64) -> Data {
    let labels: Vec<String> = [
        "North Atlantic distribution centre",
        "South Pacific warehouse and depot",
        "Central European logistics hub",
        "Eastern seaboard fulfilment",
        "Western mountains regional office",
        "Gulf coast processing terminal",
        "Northern plains grain elevator",
        "Great Lakes shipping exchange",
        "Southwest desert solar farm",
        "Pacific northwest timber yard",
        "Mid-Atlantic data centre campus",
        "Caribbean island transshipment port",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let codes = (0..12).collect();
    let counts = (0..12).map(|_| r.below(1000) as f64).collect();
    Data::from_table(table(vec![
        Column::categories("region", labels, codes).role(Role::Category),
        Column::numbers("count", ColumnType::I64, counts).role(Role::Value),
    ]))
}

/// FNV-1a 64 of a data's names, types, labels and value bits: equal
/// digests mean bitwise-equal generations (REQ-FIX-010).
pub fn digest(data: &Data) -> u64 {
    struct Fnv(u64);
    impl Fnv {
        fn bytes(&mut self, b: &[u8]) {
            for &x in b {
                self.0 = (self.0 ^ x as u64).wrapping_mul(0x0000_0100_0000_01B3);
            }
        }
        fn text(&mut self, s: &str) {
            self.bytes(s.as_bytes());
            self.bytes(&[0xFF]);
        }
    }
    let mut h = Fnv(0xCBF2_9CE4_8422_2325);
    for (name, t) in data.tables() {
        h.text(name);
        h.bytes(&(t.len() as u64).to_le_bytes());
        for c in t.columns() {
            h.text(&c.name);
            h.text(c.kind.name());
            h.text(c.role.map_or("", Role::name));
            h.text(c.zone.as_deref().unwrap_or(""));
            h.bytes(&c.origin.to_le_bytes());
            c.categories.iter().for_each(|s| h.text(s));
            match &c.values {
                Values::Num(v) => v.iter().for_each(|x| h.bytes(&x.to_bits().to_le_bytes())),
                Values::Codes(v) => v.iter().for_each(|x| h.bytes(&x.to_le_bytes())),
                Values::Text(v) => v.iter().for_each(|s| h.text(s)),
            }
        }
    }
    for (name, m) in data.matrices() {
        h.text(name);
        h.bytes(&(m.rows() as u64).to_le_bytes());
        h.bytes(&(m.cols() as u64).to_le_bytes());
        m.row_labels.iter().chain(&m.col_labels).for_each(|s| h.text(s));
        m.values().iter().for_each(|x| h.bytes(&x.to_bits().to_le_bytes()));
    }
    h.0
}

/// Whether `data` is `f`'s shape with every channel the shape needs.
pub fn fits(f: &Fixture, data: &Data) -> bool {
    let none = Encode::default();
    match f.shape {
        Shape::Matrix => data.first_matrix().is_some(),
        s => data.first_table().is_some_and(|t| s.channels()[0].iter().all(|ch| t.channel(ch, &none).is_ok())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digests of every fixture, in [`FIXTURES`] order, as generated on
    /// aarch64-apple-darwin; a wasm32 build gives the same (REQ-FIX-010).
    const DIGESTS: [u64; 23] = [
        0xd6c2a2915639c55a,
        0xfc226687bc53276d,
        0x456d3bb22b74b730,
        0x36377741c1124b34,
        0x54846d986083c975,
        0x9b8be18f9826f711,
        0xa99f28ab7212d7b8,
        0x310909b4ed538b13,
        0x4ac6f65a30488360,
        0xe9e4b37450495b41,
        0x38363d72c5de708c,
        0xa6b1143d02568370,
        0x7910cae157a11c3a,
        0x5fa7dbc660ab49b4,
        0xff2ec96ab2b3ab19,
        0x9741b40315ce9fda,
        0xcc0e00566f615e79,
        0x54ef4337d06b7398,
        0x19bd02298371923c,
        0x33c711bba9b49e2d,
        0xe03019c84babb3a1,
        0x36962998a8fbe4a9,
        0x10531a41e31ca3de,
    ];

    /// REQ-FIX-010: two generations are equal bit for bit, and equal the
    /// pinned digests.
    #[test]
    fn generations_are_bitwise_equal() {
        let mut found = Vec::new();
        for f in &FIXTURES {
            let (a, b) = (f.generate(), f.generate());
            assert!(a.bitwise_eq(&b), "{} differs between generations", f.name);
            assert_eq!(digest(&a), digest(&b));
            found.push(digest(&a));
        }
        assert_eq!(found, DIGESTS, "digests: {found:#018x?}");
    }

    /// REQ-FIX-010: the plan's 23 fixtures, each of its shape and rows.
    #[test]
    fn fixtures_match_the_plan_table() {
        assert_eq!(FIXTURES.len(), 23);
        for (i, f) in FIXTURES.iter().enumerate() {
            assert!(FIXTURES[..i].iter().all(|g| g.name != f.name && g.seed != f.seed), "{} repeats", f.name);
            assert_eq!(fixture(f.name), Some(f));
            let d = f.generate();
            assert!(fits(f, &d), "{} is not {}", f.name, f.shape);
            assert_eq!(d.shape(&[f.shape], &Encode::default()), Ok(f.shape));
            match f.shape {
                Shape::Matrix => {
                    let m = d.first_matrix().unwrap();
                    assert_eq!((m.rows(), m.cols()), (f.rows, f.rows), "{}", f.name);
                }
                _ => {
                    let t = d.first_table().unwrap();
                    let ys = t.channels("y", &Encode::default()).map_or(1, |v| v.len());
                    let named = t.channel("series", &Encode::default()).ok().map(|i| t.column(i).unwrap().categories.len());
                    let series = named.unwrap_or(if f.shape == Shape::Series { ys } else { 1 });
                    assert_eq!(series, f.series, "{} series", f.name);
                    let len = if named.is_some() { f.rows * f.series } else { f.rows };
                    assert_eq!(t.len(), len, "{} rows", f.name);
                }
            }
        }
        assert!(generate("nope").is_none());
    }

    #[test]
    fn fixtures_have_their_features() {
        let y = |name: &str| generate(name).unwrap().first_table().unwrap().by_name("y").unwrap().nums().unwrap().to_vec();
        let gaps = y("nan-gaps");
        assert_eq!(gaps.iter().filter(|v| v.is_nan()).count(), 2_500);
        assert!(gaps[1499].is_finite() && gaps[1500].is_nan() && gaps[1999].is_nan() && gaps[2000].is_finite());
        let inf = y("inf");
        assert!(inf.contains(&f64::INFINITY) && inf.contains(&f64::NEG_INFINITY));
        let huge = y("huge-range");
        assert_eq!((huge[0], huge[999]), (1e-300, 1e300));
        assert!(huge.windows(2).all(|w| w[0] > 0.0 && w[0].is_finite()));
        let tiny = y("tiny-range");
        assert!(tiny.windows(2).all(|w| w[1] > w[0]) && tiny[999] - tiny[0] < 1e-12);
        assert!(y("negative").iter().all(|&v| v <= 0.0));
        let zc = y("zero-crossing");
        assert!(zc.iter().any(|&v| v > 5.0) && zc.iter().any(|&v| v < -5.0));
        let steps = y("steps-10k");
        assert!(steps.windows(2).filter(|w| w[0] != w[1]).count() < 200);

        let unsorted = generate("unsorted-x").unwrap();
        let t = unsorted.first_table().unwrap();
        assert!(!t.is_sorted_by(0));
        let (sorted, _) = t.sorted_by(0).unwrap();
        assert_eq!(sorted.column(0).unwrap().nums().unwrap(), (0..1000).map(|i| i as f64).collect::<Vec<_>>().as_slice());
        let dup = generate("duplicate-x").unwrap();
        let x = dup.first_table().unwrap().column(0).unwrap().nums().unwrap().to_vec();
        assert!(x.chunks(3).all(|c| c.iter().all(|&v| v == c[0])));

        let wide = generate("labels-wide").unwrap();
        let s = wide.first_table().unwrap().by_name("series").unwrap();
        assert!(s.categories.iter().any(|l| l.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))));
        assert!(s.categories.iter().any(|l| l.chars().any(|c| ('\u{300}'..='\u{36F}').contains(&c))));
        assert!(wide.first_table().unwrap().is_sorted_by(0));

        let days = generate("ohlcv-days").unwrap();
        let t = days.first_table().unwrap();
        let col = |n: &str| t.by_name(n).unwrap().nums().unwrap();
        let ts = col("t");
        assert!(ts.iter().all(|&ms| {
            let wd = ((ms / DAY_MS) as i64 + 4) % 7;
            (1..=5).contains(&wd)
        }));
        assert!(ts.windows(2).any(|w| w[1] - w[0] == 3.0 * DAY_MS), "weekend gaps");
        let (o, c, l, h) = (col("open"), col("close"), col("low"), col("high"));
        assert_eq!(c[LIMIT_DOWN_ROW], o[LIMIT_DOWN_ROW] * 0.9);
        assert_eq!((l[LIMIT_DOWN_ROW], h[LIMIT_DOWN_ROW]), (c[LIMIT_DOWN_ROW], o[LIMIT_DOWN_ROW]));
        assert_eq!(col("volume")[ZERO_VOLUME_ROW], 0.0);
        assert!((0..500).all(|i| l[i] <= o[i].min(c[i]) && h[i] >= o[i].max(c[i])));

        let dst = generate("timestamps-dst").unwrap();
        let t = dst.first_table().unwrap().by_name("t").unwrap();
        assert_eq!(t.zone.as_deref(), Some("America/New_York"));
        // 2026-11-01T06:00:00Z, when New York leaves daylight time, is a row.
        let change = (crate::data::rfc3339::days_from_civil(2026, 11, 1) * 86_400 + 6 * 3600) as f64 * 1000.0;
        assert_eq!(t.nums().unwrap().iter().position(|&v| v == change), Some(750));

        let cats = generate("categorical-12").unwrap();
        assert!(cats.first_table().unwrap().column(0).unwrap().categories.iter().all(|l| l.chars().count() > 20));
        assert_eq!(cats.shape(&[Shape::Counts], &Encode::default()), Ok(Shape::Counts));
    }

    #[test]
    fn sine_is_close_to_std_and_exact_where_it_must_be() {
        assert_eq!(sin_turns(0.0), 0.0);
        assert_eq!(sin_turns(0.25), 1.0);
        assert_eq!(sin_turns(-0.25), -1.0);
        assert!(sin_turns(f64::NAN).is_nan() && sin_turns(f64::INFINITY).is_nan());
        let mut worst = 0.0f64;
        for i in -20_000..=20_000 {
            let t = i as f64 / 20_000.0;
            worst = worst.max((sin_turns(t) - (t * std::f64::consts::TAU).sin()).abs());
        }
        assert!(worst < 1e-15, "{worst}");
        for i in -4096..4096 {
            let t = i as f64 / 1024.0;
            assert_eq!(sin_turns(t + 16.0).to_bits(), sin_turns(t).to_bits(), "{t}");
        }
        let mut r = SplitMix64::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF, "SplitMix64's published first output for seed 0");
        assert!((0..1000).all(|_| (0.0..1.0).contains(&r.next_f64())));
    }
}
