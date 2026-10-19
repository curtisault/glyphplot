//! Native side of the bench: writes the fixtures (CSV, JSON, Arrow IPC) to
//! target/fixtures, then times parsing, the ring and the downsamplers natively.
//! `cargo run --release --features native --bin native`

use glyphplot_bench_data::core::{self, Points, Ring};
use std::fmt::Write as _;
use std::time::Instant;

const REPS: usize = 7;

/// Min and median of `REPS` runs of `f`, in ms.
fn time<R>(mut f: impl FnMut() -> R) -> (f64, f64) {
    let mut ms: Vec<f64> = (0..REPS)
        .map(|_| {
            let s = Instant::now();
            std::hint::black_box(f());
            s.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    (ms[0], ms[REPS / 2])
}

fn iso(ms: f64) -> String {
    let s = (ms / 1000.0) as i64;
    let (days, rem) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // civil_from_days (Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z", rem / 3600, rem / 60 % 60, rem % 60)
}

fn arrow(t: &[f64], v: &[f64]) -> Vec<u8> {
    use arrow_array::{Float64Array, RecordBatch, TimestampMillisecondArray};
    use arrow_schema::{DataType, Field, Schema, TimeUnit};
    use std::sync::Arc;
    let schema = Arc::new(Schema::new(vec![
        Field::new("t", DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())), false),
        Field::new("v", DataType::Float64, true),
    ]));
    let mut buf = Vec::new();
    let mut w = arrow_ipc::writer::StreamWriter::try_new(&mut buf, &schema).unwrap();
    // Batches of 64 Ki rows, as a feed or a file reader would write them.
    for (tc, vc) in t.chunks(65_536).zip(v.chunks(65_536)) {
        let ta = TimestampMillisecondArray::from(tc.iter().map(|&x| x as i64).collect::<Vec<_>>()).with_timezone("UTC");
        let va = Float64Array::from(vc.to_vec());
        w.write(&RecordBatch::try_new(schema.clone(), vec![Arc::new(ta), Arc::new(va)]).unwrap()).unwrap();
    }
    w.finish().unwrap();
    drop(w);
    buf
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/fixtures");
    std::fs::create_dir_all(&dir).unwrap();
    println!("| what | rows | min ms | median ms |\n|---|---|---|---|");
    for n in [1_000usize, 100_000, 1_000_000] {
        let (t, v) = core::walk(n);
        let mut csv = String::from("t,v\n");
        let mut csv_iso = String::from("t,v\n");
        let mut cols = String::from("{\"t\":[");
        let mut rows = String::from("[");
        for i in 0..n {
            writeln!(csv, "{},{}", t[i], v[i]).unwrap();
            writeln!(csv_iso, "{},{}", iso(t[i]), v[i]).unwrap();
            write!(rows, "{}{{\"t\":{},\"v\":{}}}", if i > 0 { "," } else { "" }, t[i], v[i]).unwrap();
        }
        rows.push(']');
        cols += &t.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        cols += "],\"v\":[";
        cols += &v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        cols += "]}";
        let ipc = arrow(&t, &v);
        std::fs::write(dir.join(format!("walk-{n}.csv")), &csv).unwrap();
        std::fs::write(dir.join(format!("walk-{n}-iso.csv")), &csv_iso).unwrap();
        std::fs::write(dir.join(format!("walk-{n}-cols.json")), &cols).unwrap();
        std::fs::write(dir.join(format!("walk-{n}-rows.json")), &rows).unwrap();
        std::fs::write(dir.join(format!("walk-{n}.arrows")), &ipc).unwrap();

        let mut out: Vec<Vec<f64>> = Vec::new();
        let row = |what: &str, r: (f64, f64)| println!("| {what} | {n} | {:.3} | {:.3} |", r.0, r.1);
        row("copy two columns (to_vec)", time(|| (t.to_vec(), v.to_vec())));
        row("csv, epoch ms", time(|| core::parse_csv(&csv, &mut out).unwrap()));
        assert_eq!(out[1].len(), n);
        row("csv, RFC 3339", time(|| core::parse_csv(&csv_iso, &mut out).unwrap()));
        assert_eq!(out[0][n - 1], t[n - 1]);
        row("json columns (serde_json)", time(|| core::parse_json_columns(&cols, &mut out).unwrap()));
        row("json rows (serde_json)", time(|| core::parse_json_rows(&rows, &mut out).unwrap()));
        row("arrow ipc, by hand", time(|| core::ipc::read(&ipc).unwrap()));
        let c = core::ipc::read(&ipc).unwrap();
        assert_eq!(c[0].values, t);
        assert_eq!(c[1].values, v);
        row("arrow ipc, arrow-rs", time(|| core::read_arrow_rs(&ipc, &mut out).unwrap()));
        assert_eq!(out[1], v);
        println!("<!-- sizes: csv {} B, csv-iso {} B, json-cols {} B, json-rows {} B, arrow {} B -->", csv.len(), csv_iso.len(), cols.len(), rows.len(), ipc.len());
    }

    // Downsampling 1M points, the whole series in view, then a ring that has wrapped.
    let n = 1_000_000;
    let (t, v) = core::walk(n);
    let (x0, x1) = (t[0], t[n - 1] + 1.0);
    let mut p = Points::default();
    println!("\n| downsampler | width | points out | min ms | median ms |\n|---|---|---|---|---|");
    for w in [160usize, 640] {
        for (name, how) in [("min-max", 0), ("M4", 1), ("LTTB", 2)] {
            let segs = [(&t[..], &v[..])];
            let r = time(|| match how {
                2 => core::lttb(&segs, x0, x1, w, &mut p),
                m => core::columns(&segs, x0, x1, w, m == 1, &mut p),
            });
            println!("| {name} | {w} | {} | {:.3} | {:.3} |", p.x.len(), r.0, r.1);
        }
    }
    // What each keeps: per dot column, does the output reach the column's true min and max?
    println!("\n| downsampler | width | columns whose true max or min no output point in that column reaches |\n|---|---|---|");
    for w in [160usize, 640] {
        let scale = w as f64 / (x1 - x0);
        let mut lo = vec![f64::INFINITY; w];
        let mut hi = vec![f64::NEG_INFINITY; w];
        for i in 0..n {
            let c = ((t[i] - x0) * scale) as usize;
            lo[c] = lo[c].min(v[i]);
            hi[c] = hi[c].max(v[i]);
        }
        for (name, how) in [("min-max", 0), ("M4", 1), ("LTTB", 2)] {
            let segs = [(&t[..], &v[..])];
            match how {
                2 => core::lttb(&segs, x0, x1, w, &mut p),
                m => core::columns(&segs, x0, x1, w, m == 1, &mut p),
            }
            let mut olo = vec![f64::INFINITY; w];
            let mut ohi = vec![f64::NEG_INFINITY; w];
            for k in 0..p.x.len() {
                let c = ((p.x[k] - x0) * scale) as usize;
                olo[c] = olo[c].min(p.y[k]);
                ohi[c] = ohi[c].max(p.y[k]);
            }
            let missed = (0..w).filter(|&c| olo[c] != lo[c] || ohi[c] != hi[c]).count();
            println!("| {name} | {w} | {missed} of {w} |");
        }
    }
    // A 1M ring after 1.5M samples, so it holds two segments.
    let mut ring = Ring::new(n);
    let (t2, v2) = core::walk(n + n / 2);
    ring.push_batch(&t2, &v2);
    let segs = ring.segments();
    let (x0, x1) = (t2[n / 2], t2[n + n / 2 - 1] + 1.0);
    for w in [160usize, 640] {
        let r = time(|| core::columns(&segs, x0, x1, w, true, &mut p));
        println!("| M4 over a wrapped 1M ring | {w} | {} | {:.3} | {:.3} |", p.x.len(), r.0, r.1);
    }
    // A zoomed view: 1% of the series.
    let (x0z, x1z) = (t[n / 2], t[n / 2 + n / 100]);
    let r = time(|| core::columns(&[(&t[..], &v[..])], x0z, x1z, 640, true, &mut p));
    println!("| M4, view of 10k of 1M (binary search then scan) | 640 | {} | {:.3} | {:.3} |", p.x.len(), r.0, r.1);

    // Ring appends: 10 s of 1 kHz singles, and 10 s of 10 Hz batches of 1000.
    println!("\n| ring append (cap 1M) | calls | total min ms | median ms | per call us (median) |\n|---|---|---|---|---|");
    let mut ring = Ring::new(n);
    let r = time(|| {
        for i in 0..10_000 {
            ring.push(t[i], v[i]);
        }
    });
    println!("| push one, 1 kHz x 10 s | 10000 | {:.3} | {:.3} | {:.4} |", r.0, r.1, r.1 * 1e3 / 1e4);
    let r = time(|| {
        for k in 0..100 {
            ring.push_batch(&t[k * 1000..(k + 1) * 1000], &v[k * 1000..(k + 1) * 1000]);
        }
    });
    println!("| push batch of 1000, 10 Hz x 10 s | 100 | {:.3} | {:.3} | {:.4} |", r.0, r.1, r.1 * 1e3 / 100.0);

    // Planet positions.
    let jd = 2_461_331.5; // 2026-10-18 0h (JD; TT and UT not told apart here)
    let r = time(|| {
        let e = core::kepler(2, jd);
        (0..8).map(|k| core::ra_dec(core::kepler(k, jd), e)).collect::<Vec<_>>()
    });
    println!("\n| planets, Kepler (8 bodies) | {:.4} ms min, {:.4} median |", r.0, r.1);
    let r = time(|| {
        let e = core::vsop(2, jd);
        (0..8).map(|k| core::ra_dec(core::vsop(k, jd), e)).collect::<Vec<_>>()
    });
    println!("| planets, VSOP87A (8 bodies) | {:.4} ms min, {:.4} median |", r.0, r.1);
    let names = ["Mercury", "Venus", "EMB", "Mars", "Jupiter", "Saturn", "Uranus", "Neptune"];
    let (ek, ev) = (core::kepler(2, jd), core::vsop(2, jd));
    println!("\n| body | Kepler RA, Dec | VSOP87A RA, Dec | separation arcsec |\n|---|---|---|---|");
    for k in [0usize, 1, 3, 4, 5, 6, 7] {
        let (a, b) = core::ra_dec(core::kepler(k, jd), ek);
        let (c, d) = core::ra_dec(core::vsop(k, jd), ev);
        let (a1, b1, c1, d1) = (a.to_radians(), b.to_radians(), c.to_radians(), d.to_radians());
        let sep = (b1.sin() * d1.sin() + b1.cos() * d1.cos() * (a1 - c1).cos()).clamp(-1.0, 1.0).acos().to_degrees() * 3600.0;
        println!("| {} | {a:.4}, {b:.4} | {c:.4}, {d:.4} | {sep:.1} |", names[k]);
    }
}
