//! The ports against their references' captured output, bit for bit
//! (ref/out, written by `cargo run --release --example capture` then
//! `node ref/capture.mjs`). Prints every mismatch count rather than stopping
//! at the first.
use glyphplot_testing_bench::{lttb, ticks};

fn out(name: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ref/out").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("{} missing: run the capture first (README.md)", p.display()))
}

#[test]
fn ticks_match_d3_array_bit_for_bit() {
    let (input, want) = (out("ticks-in.txt"), out("ticks-expected.txt"));
    let mut bad = vec![];
    let mut n = 0;
    for (line, want) in input.lines().zip(want.lines()) {
        n += 1;
        let a: Vec<f64> = line.split(' ').map(|v| v.parse().unwrap()).collect();
        let got = ticks(a[0], a[1], a[2]);
        let want: Vec<f64> = want.split(' ').filter(|s| !s.is_empty()).map(|v| v.parse().unwrap()).collect();
        let same = got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| g.to_bits() == w.to_bits());
        if !same {
            bad.push(format!("ticks({line}): got {got:?}, d3 {want:?}"));
        }
    }
    println!("{} of {n} ticks cases differ from d3-array", bad.len());
    for b in bad.iter().take(10) {
        println!("  {b}");
    }
    assert!(bad.is_empty());
}

#[test]
fn lttb_matches_the_reference() {
    let mut bad = vec![];
    for line in out("lttb-expected.txt").lines() {
        let mut f = line.split(' ');
        let (name, threshold): (&str, usize) = (f.next().unwrap(), f.next().unwrap().parse().unwrap());
        let want: Vec<usize> = f.map(|v| v.parse().unwrap()).collect();
        let pts: Vec<(f64, f64)> = out(&format!("lttb-{name}.txt"))
            .lines()
            .map(|l| {
                let (x, y) = l.split_once(' ').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        let (x, y): (Vec<f64>, Vec<f64>) = pts.into_iter().unzip();
        if lttb(&x, &y, threshold) != want {
            bad.push(format!("{name} at {threshold}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
