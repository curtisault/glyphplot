//! Throwaway discovery bench for discovery/charts.md.
//!
//! `cargo run --release -- time` times three ways of drawing one line series
//! of N points into a braille plot of C x R cells:
//!   ratatui:   ratatui-widgets' Chart with Marker::Braille and GraphType::Line,
//!              which draws one canvas line per pair of points (the prior art).
//!   segments:  the same idea without ratatui: one Bresenham line per pair of
//!              points into a braille bitmask grid.
//!   minmax:    one pass over the points keeping first, last, min and max per
//!              dot column, then one vertical span per dot column (M4-style,
//!              as uPlot's paths/linear.js and Chart.js's min-max decimation do).
//! `cargo run --release -- time noise` does the same with uniform noise for y.
//! `cargo run --release -- candles` prints candle glyph samples.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::symbols::Marker;
use ratatui_core::widgets::Widget;
use ratatui_widgets::chart::{Axis, Chart, Dataset, GraphType};
use std::hint::black_box;
use std::time::Instant;

thread_local! {
    static NOISE: bool = std::env::args().any(|a| a == "noise");
}

/// A deterministic random walk (or, with `noise`, uniform noise) of n points, x = i.
fn walk(n: usize) -> Vec<(f64, f64)> {
    let mut s: u32 = 0x9e37_79b9;
    let mut y = 0.0f64;
    (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let u = (s as f64 / u32::MAX as f64) - 0.5;
            // `noise` makes every sample independent, so each segment spans much of the height.
            if NOISE.with(|n| *n) {
                y = u;
            } else {
                y += u;
            }
            (i as f64, y)
        })
        .collect()
}

fn bounds(p: &[(f64, f64)]) -> (f64, f64) {
    p.iter().fold((f64::MAX, f64::MIN), |(a, b), &(_, y)| (a.min(y), b.max(y)))
}

/// A braille canvas of `cols x rows` cells, 2 x 4 dots a cell. Each cell keeps
/// the bits of its lit dots as Unicode orders them (U+2800 plus the bits):
/// dots 1-2-3 down the left are 0x01, 0x02, 0x04 and dots 4-5-6 down the right
/// 0x08, 0x10, 0x20; the bottom row, dots 7 and 8, is 0x40 and 0x80.
struct Dots {
    cols: usize,
    rows: usize,
    mask: Vec<u8>,
}

impl Dots {
    fn new(cols: usize, rows: usize) -> Self {
        let mut mask = Vec::with_capacity(cols * rows);
        mask.resize(cols * rows, 0u8);
        Dots { cols, rows, mask }
    }
    #[inline]
    fn set(&mut self, x: i64, y: i64) {
        let (w, h) = (2 * self.cols as i64, 4 * self.rows as i64);
        if !(0..w).contains(&x) || !(0..h).contains(&y) {
            return;
        }
        let (col, row) = ((x & 1) as u32, (y & 3) as u32);
        let bit = if row == 3 { 0x40u8 << col } else { 1u8 << (row + 3 * col) };
        self.mask[(x >> 1) as usize + (y >> 2) as usize * self.cols] |= bit;
    }
    fn vspan(&mut self, x: i64, y0: i64, y1: i64) {
        for y in y0.min(y1)..=y0.max(y1) {
            self.set(x, y);
        }
    }
    /// Bresenham's line in its midpoint form: step one dot along the longer
    /// axis each time, and one along the shorter when the decision term is not negative.
    fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64) {
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        // Walk in (major, minor) coordinates and swap back when plotting.
        let (a0, b0, a1, b1) = if steep { (y0, x0, y1, x1) } else { (x0, y0, x1, y1) };
        let (da, db) = ((a1 - a0).abs(), (b1 - b0).abs());
        let (sa, sb) = ((a1 - a0).signum(), (b1 - b0).signum());
        let mut decide = 2 * db - da;
        let mut b = b0;
        for k in 0..=da {
            let a = a0 + k * sa;
            if steep {
                self.set(b, a);
            } else {
                self.set(a, b);
            }
            if decide >= 0 {
                b += sb;
                decide -= 2 * da;
            }
            decide += 2 * db;
        }
    }
    /// Code points, a space for an empty cell.
    fn cells(&self, out: &mut [u32]) {
        for (i, o) in out.iter_mut().enumerate().take(self.mask.len()) {
            let m = u32::from(self.mask[i]);
            *o = if m != 0 { 0x2800 | m } else { u32::from(' ') };
        }
    }
    fn lit(&self) -> usize {
        self.mask.iter().fold(0, |n, &m| n + usize::from(m != 0))
    }
}

fn to_dot(v: f64, lo: f64, hi: f64, n: usize) -> i64 {
    ((v - lo) / (hi - lo) * (n - 1) as f64).round() as i64
}

fn segments(p: &[(f64, f64)], cols: usize, rows: usize, out: &mut [u32]) -> usize {
    let (lo, hi) = bounds(p);
    let (w, h) = (cols * 2, rows * 4);
    let x1 = p.last().unwrap().0;
    let mut d = Dots::new(cols, rows);
    for s in p.windows(2) {
        let (ax, ay) = (to_dot(s[0].0, 0.0, x1, w), h as i64 - 1 - to_dot(s[0].1, lo, hi, h));
        let (bx, by) = (to_dot(s[1].0, 0.0, x1, w), h as i64 - 1 - to_dot(s[1].1, lo, hi, h));
        d.line(ax, ay, bx, by);
    }
    d.cells(out);
    d.lit()
}

fn minmax(p: &[(f64, f64)], cols: usize, rows: usize, out: &mut [u32]) -> usize {
    let (lo, hi) = bounds(p);
    let (w, h) = (cols * 2, rows * 4);
    let x1 = p.last().unwrap().0;
    // first, last, min, max per dot column, in dot rows; i64::MIN = empty.
    let mut acc = vec![(i64::MIN, 0i64, i64::MAX, i64::MIN); w];
    for &(x, y) in p {
        let c = to_dot(x, 0.0, x1, w) as usize;
        let r = h as i64 - 1 - to_dot(y, lo, hi, h);
        let a = &mut acc[c];
        if a.0 == i64::MIN {
            a.0 = r;
        }
        a.1 = r;
        a.2 = a.2.min(r);
        a.3 = a.3.max(r);
    }
    let mut d = Dots::new(cols, rows);
    let mut prev: Option<(i64, i64)> = None;
    for (c, a) in acc.iter().enumerate() {
        if a.0 == i64::MIN {
            continue;
        }
        let c = c as i64;
        if let Some((pc, pl)) = prev {
            d.line(pc, pl, c, a.0);
        }
        d.vspan(c, a.2, a.3);
        prev = Some((c, a.1));
    }
    d.cells(out);
    d.lit()
}

fn ratatui(p: &[(f64, f64)], cols: usize, rows: usize, buf: &mut Buffer) -> usize {
    let (lo, hi) = bounds(p);
    let x1 = p.last().unwrap().0;
    buf.reset();
    let chart = Chart::new(vec![Dataset::default().marker(Marker::Braille).graph_type(GraphType::Line).data(p)])
        .x_axis(Axis::default().bounds([0.0, x1]))
        .y_axis(Axis::default().bounds([lo, hi]));
    chart.render(Rect::new(0, 0, cols as u16, rows as u16), buf);
    buf.content().iter().filter(|c| c.symbol() != " ").count()
}

fn stats(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2])
}

fn time() {
    let reps = 5;
    println!("| method | points | cells | min ms | median ms | lit cells |");
    println!("|---|---:|---|---:|---:|---:|");
    for &n in &[1_000usize, 10_000, 100_000, 1_000_000] {
        let p = walk(n);
        for &(cols, rows) in &[(80usize, 24usize), (320, 120)] {
            let mut out = vec![0u32; cols * rows];
            let mut buf = Buffer::empty(Rect::new(0, 0, cols as u16, rows as u16));
            for (name, which) in [("ratatui", 0), ("segments", 1), ("minmax", 2)] {
                let mut ts = vec![];
                let mut lit = 0;
                for _ in 0..reps {
                    let t = Instant::now();
                    lit = match which {
                        0 => ratatui(black_box(&p), cols, rows, &mut buf),
                        1 => segments(black_box(&p), cols, rows, &mut out),
                        _ => minmax(black_box(&p), cols, rows, &mut out),
                    };
                    black_box(&out);
                    ts.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                let (mn, md) = stats(ts);
                println!("| {name} | {n} | {cols}x{rows} | {mn:.3} | {md:.3} | {lit} |");
            }
        }
    }
}

/// Synthetic OHLC candles.
fn ohlc(n: usize) -> Vec<[f64; 4]> {
    let mut s: u32 = 7;
    let mut rnd = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s as f64 / u32::MAX as f64
    };
    let mut c = 100.0;
    (0..n)
        .map(|_| {
            let o = c;
            c = o + (rnd() - 0.5) * 3.0;
            let h = o.max(c) + rnd() * 1.5;
            let l = o.min(c) - rnd() * 1.5;
            [o, h, l, c]
        })
        .collect()
}

/// One candle a cell, row resolution: wick `│`, body `█` (plotext's candle style).
fn candles_block(cs: &[[f64; 4]], rows: usize) -> Vec<String> {
    let (lo, hi) = cs.iter().fold((f64::MAX, f64::MIN), |(a, b), c| (a.min(c[2]), b.max(c[1])));
    let r = |v: f64| rows - 1 - to_dot(v, lo, hi, rows) as usize;
    let mut g = vec![vec![' '; cs.len()]; rows];
    for (x, c) in cs.iter().enumerate() {
        for row in g.iter_mut().take(r(c[2]) + 1).skip(r(c[1])) {
            row[x] = '│';
        }
        for row in g.iter_mut().take(r(c[0].min(c[3])) + 1).skip(r(c[0].max(c[3]))) {
            row[x] = '█';
        }
    }
    g.into_iter().map(|r| r.into_iter().collect()).collect()
}

/// One candle a cell in braille: the wick is the left dot column, the body
/// both columns; four price levels a row.
fn candles_braille(cs: &[[f64; 4]], rows: usize) -> Vec<String> {
    let (lo, hi) = cs.iter().fold((f64::MAX, f64::MIN), |(a, b), c| (a.min(c[2]), b.max(c[1])));
    let h = rows * 4;
    let r = |v: f64| h as i64 - 1 - to_dot(v, lo, hi, h);
    let mut d = Dots::new(cs.len(), rows);
    for (x, c) in cs.iter().enumerate() {
        let x = x as i64 * 2;
        d.vspan(x, r(c[1]), r(c[2]));
        d.vspan(x + 1, r(c[0]), r(c[3]));
    }
    let mut out = vec![0u32; cs.len() * rows];
    d.cells(&mut out);
    out.chunks(cs.len()).map(|row| row.iter().map(|&u| char::from_u32(u).unwrap()).collect()).collect()
}

/// One candle every other cell, body ends at eighth-of-a-row: the cell holding
/// the body's top fills from the bottom (`▁`..`▇`), the cell holding its
/// bottom fills from the top (`▔ 🮂 🮃 ▀ 🮄 🮅 🮆`), the wick `│` above and below.
fn candles_eighths(cs: &[[f64; 4]], rows: usize) -> Vec<String> {
    const LOWER: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    const UPPER: [char; 8] = ['▔', '\u{1FB82}', '\u{1FB83}', '▀', '\u{1FB84}', '\u{1FB85}', '\u{1FB86}', '█'];
    let (lo, hi) = cs.iter().fold((f64::MAX, f64::MIN), |(a, b), c| (a.min(c[2]), b.max(c[1])));
    let h = rows * 8;
    // Eighth-rows from the top.
    let e = |v: f64| h as i64 - to_dot(v, lo, hi, h + 1);
    let mut g = vec![vec![' '; cs.len() * 2]; rows];
    for (i, c) in cs.iter().enumerate() {
        let x = i * 2;
        let (t, b) = (e(c[0].max(c[3])), e(c[0].min(c[3])).max(e(c[0].max(c[3])) + 1));
        let (wt, wb) = (e(c[1]), e(c[2]));
        for (r, row) in g.iter_mut().enumerate() {
            let (r0, r1) = (r as i64 * 8, r as i64 * 8 + 8);
            let cover = (b.min(r1) - t.max(r0)).max(0);
            row[x] = if cover >= 8 {
                '█'
            } else if cover > 0 && t > r0 {
                LOWER[cover as usize - 1]
            } else if cover > 0 {
                UPPER[cover as usize - 1]
            } else if wt < r1 && wb > r0 {
                '│'
            } else {
                ' '
            };
        }
    }
    g.into_iter().map(|r| r.into_iter().collect()).collect()
}

/// Braille with a blank cell between candles.
fn candles_braille_gap(cs: &[[f64; 4]], rows: usize) -> Vec<String> {
    candles_braille(cs, rows).into_iter().map(|l| l.chars().flat_map(|c| [c, ' ']).collect()).collect()
}

fn candles() {
    let cs = ohlc(60);
    let half = &cs[..30];
    println!("one candle every other cell, eighth-row body ends:");
    for l in candles_eighths(half, 10) {
        println!("  {l}");
    }
    println!("one candle every other cell, braille:");
    for l in candles_braille_gap(half, 10) {
        println!("  {l}");
    }
    println!("one candle a cell, blocks (row resolution):");
    for l in candles_block(&cs, 10) {
        println!("  {l}");
    }
    println!("one candle a cell, braille (quarter-row resolution):");
    for l in candles_braille(&cs, 10) {
        println!("  {l}");
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("candles") => candles(),
        _ => time(),
    }
}
