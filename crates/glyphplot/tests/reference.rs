//! The core's ports against output captured from the pinned originals by
//! `scripts/fixtures.mjs` (`mise run fixtures`) into `tests/fixtures/*.txt`,
//! compared bit for bit: every number by its bits, every label by its text
//! (REQ-REF-010). Each test reads every case, prints the first mismatches
//! and how many there were, and fails on any.

use glyphplot::downsample::lttb;
use glyphplot::fixtures::SplitMix64;
use glyphplot::format::{js, tick_format, Locale, NumberFormat, Specifier};
use glyphplot::locale::ZoneTable;
use glyphplot::scale::{log_nice, nice};
use glyphplot::ticks::{log_ticks, tick_increment, tick_step, ticks, LogTickFormat};
use glyphplot::time::{strftime, utc_nice, utc_tick_label, utc_ticks, Zone};

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/").to_string() + name;
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}; run `mise run fixtures`"))
}

/// The cases of a fixture file: every line but comments.
fn cases(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|l| !l.is_empty() && !l.starts_with('#'))
}

/// A number as `String(x)` wrote it; `str::parse` reads the shortest form
/// back to the same bits.
fn num(s: &str) -> f64 {
    s.parse().unwrap_or_else(|_| panic!("not a number: {s:?}"))
}

fn nums(s: &str) -> Vec<f64> {
    s.split(' ').filter(|w| !w.is_empty()).map(num).collect()
}

/// Bitwise equality, NaN equal to NaN.
fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()))
}

/// The mismatches of one test.
struct Report {
    name: &'static str,
    cases: usize,
    bad: Vec<String>,
}

impl Report {
    fn new(name: &'static str) -> Report {
        Report { name, cases: 0, bad: Vec::new() }
    }

    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.cases += 1;
        if !ok {
            self.bad.push(what());
        }
    }

    fn finish(self) {
        for b in self.bad.iter().take(40) {
            eprintln!("{}: {b}", self.name);
        }
        eprintln!("{}: {} of {} cases differ", self.name, self.bad.len(), self.cases);
        assert!(self.cases > 0, "{}: no cases", self.name);
        assert!(self.bad.is_empty(), "{}: {} of {} cases differ", self.name, self.bad.len(), self.cases);
    }
}

/// `head : tail` with the head's numbers.
fn split(line: &str) -> (Vec<&str>, &str) {
    let (head, tail) = line.split_once(" : ").map_or((line.trim_end_matches(" :"), ""), |(h, t)| (h, t));
    (head.split(' ').collect(), tail)
}

/// REQ-TICK-010: d3-array 3.2.4's `ticks`, `tickIncrement` and `tickStep`.
#[test]
fn ticks_match_d3_array() {
    let text = fixture("ticks.txt");
    let mut r = Report::new("ticks");
    for line in cases(&text) {
        let (head, tail) = split(line);
        let (a, b, n) = (num(head[1]), num(head[2]), num(head[3]));
        let want = nums(tail);
        match head[0] {
            "ticks" => {
                let got = ticks(a, b, n);
                r.check(same(&got, &want), || format!("{line}\n  got {got:?}"));
            }
            "step" => {
                let got = [tick_increment(a, b, n), tick_step(a, b, n)];
                r.check(same(&got, &want), || format!("{line}\n  got {got:?}"));
            }
            other => panic!("unknown case {other}"),
        }
    }
    r.finish();
}

/// REQ-SCALE-030: d3-scale 4.0.2's `linear().nice(count)`, the domains that
/// do not settle included, and `scaleUtc().nice(count)`.
#[test]
fn nice_matches_d3_scale() {
    let text = fixture("nice.txt");
    let mut r = Report::new("nice");
    for line in cases(&text) {
        let (head, tail) = split(line);
        let (a, b, n) = (num(head[1]), num(head[2]), num(head[3]));
        let got = match head[0] {
            "linear" => nice(a, b, n),
            "utc" => utc_nice(a, b, n),
            other => panic!("unknown case {other}"),
        };
        r.check(same(&got, &nums(tail)), || format!("{line}\n  got {got:?}"));
    }
    r.finish();
}

/// REQ-TICK-020: d3-time 3.1.0's `utcTicks`.
#[test]
fn time_ticks_match_d3_time() {
    let text = fixture("time-ticks.txt");
    let mut r = Report::new("time-ticks");
    for line in cases(&text) {
        let (head, tail) = split(line);
        let got = utc_ticks(num(head[0]), num(head[1]), num(head[2]));
        r.check(same(&got, &nums(tail)), || format!("{line}\n  got {got:?}"));
    }
    r.finish();
}

/// REQ-FMT-010, REQ-FMT-020, REQ-FMT-030: JavaScript's `toFixed`,
/// `toExponential`, `toPrecision` and `toString`; d3-format 3.1.2's
/// formats; d3-scale 4.0.2's tick formats over ticks; and the UTC tick
/// labels and strftime directives of d3-scale and d3-time-format 4.1.0.
#[test]
fn format_matches_d3_format() {
    let locale = Locale::default();
    let text = fixture("format.txt");
    let mut r = Report::new("format");
    for line in cases(&text) {
        let f: Vec<&str> = line.split('\t').collect();
        let mut got = String::new();
        let want = match f[0] {
            "fixed" => {
                js::to_fixed(num(f[1]), f[2].parse().unwrap(), &mut got);
                f[3]
            }
            "exponential" => {
                js::to_exponential(num(f[1]), f[2].parse().ok(), &mut got);
                f[3]
            }
            "precision" => {
                js::to_precision(num(f[1]), f[2].parse().unwrap(), &mut got);
                f[3]
            }
            "string" => {
                js::to_string(num(f[1]), &mut got);
                f[2]
            }
            "format" => {
                let spec = Specifier::parse("format", f[1]).unwrap_or_else(|e| panic!("{e}"));
                NumberFormat::new(&spec, &locale).format_into(num(f[2]), &mut got);
                f[3]
            }
            "tick" => {
                let (a, b, n) = (num(f[1]), num(f[2]), num(f[3]));
                let spec = Specifier::parse("format", f[4]).unwrap_or_else(|e| panic!("{e}"));
                let format = tick_format(a, b, n, &spec, &locale);
                let labels: Vec<String> = ticks(a, b, n).into_iter().map(|t| format.format(t)).collect();
                let want = &f[5..];
                r.check(labels == want, || format!("{line}\n  got {labels:?}"));
                continue;
            }
            other => panic!("unknown case {other}"),
        };
        r.check(got == want, || format!("{line}\n  got {got:?}"));
    }
    let text = fixture("time-format.txt");
    for line in cases(&text) {
        let f: Vec<&str> = line.split('\t').collect();
        let mut got = String::new();
        let want = match f[0] {
            "label" => {
                utc_tick_label(num(f[1]), &locale.time, &mut got);
                f[2]
            }
            "strftime" => {
                strftime(f[1], num(f[2]), &locale.time, &mut got);
                f[3]
            }
            other => panic!("unknown case {other}"),
        };
        r.check(got == want, || format!("{line}\n  got {got:?}"));
    }
    r.finish();
}

/// REQ-SCALE-040, REQ-TICK-060: d3-scale 4.0.2's `scaleLog()` ticks, nice
/// domain and tick format, in bases 10, 2, 3 and 16, on positive and
/// negative domains either way round.
#[test]
fn log_ticks_match_d3_scale() {
    let locale = Locale::default();
    let text = fixture("log.txt");
    let mut r = Report::new("log");
    for line in cases(&text) {
        if let Some(rest) = line.strip_prefix("format\t") {
            let f: Vec<&str> = rest.split('\t').collect();
            let (base, a, b, n) = (num(f[0]), num(f[1]), num(f[2]), num(f[3]));
            let spec = (f[4] != "auto").then(|| Specifier::parse("format", f[4]).unwrap_or_else(|e| panic!("{e}")));
            let format = LogTickFormat::new([a, b], base, n, spec.as_ref(), &locale);
            let labels: Vec<String> = log_ticks(a, b, n, base).into_iter().map(|t| format.format(t)).collect();
            let want = &f[5..];
            r.check(labels == want, || format!("{line}\n  got {labels:?}"));
            continue;
        }
        let (head, tail) = split(line);
        let (base, a, b) = (num(head[1]), num(head[2]), num(head[3]));
        let got = match head[0] {
            "ticks" => log_ticks(a, b, num(head[4]), base),
            "nice" => log_nice(a, b, base).to_vec(),
            other => panic!("unknown case {other}"),
        };
        r.check(same(&got, &nums(tail)), || format!("{line}\n  got {got:?}"));
    }
    r.finish();
}

/// REQ-TZ-030: local-time ticks, nice domains and multi-scale labels over
/// each zone's table equal d3-scale 4.0.2's `scaleTime()` run under
/// `TZ=<zone>`, across daylight-saving changes; the core reads only the
/// table (REQ-TZ-010).
#[test]
fn local_ticks_match_d3_scale_time() {
    let locale = Locale::default();
    let text = fixture("zone-ticks.txt");
    let mut r = Report::new("zone-ticks");
    let mut zone = None;
    let mut zones = 0;
    for line in cases(&text) {
        if let Some(rest) = line.strip_prefix("zone ") {
            let (_, json) = rest.split_once(' ').expect("zone name table");
            let table = ZoneTable::from_json(json).unwrap_or_else(|e| panic!("{}", e.0));
            zone = Some(Zone::table(table));
            zones += 1;
            continue;
        }
        let z = zone.as_ref().expect("a zone line first");
        if let Some(rest) = line.strip_prefix("label\t") {
            let (t, want) = rest.split_once('\t').expect("label t text");
            let mut got = String::new();
            z.tick_label(num(t), &locale.time, &mut got);
            r.check(got == want, || format!("{line}\n  got {got:?}"));
            continue;
        }
        let (head, tail) = split(line);
        let (a, b, n) = (num(head[1]), num(head[2]), num(head[3]));
        let got = match head[0] {
            "ticks" => z.ticks(a, b, n),
            "nice" => z.nice(a, b, n).to_vec(),
            other => panic!("unknown case {other}"),
        };
        r.check(same(&got, &nums(tail)), || format!("{line}\n  got {got:?}"));
    }
    assert_eq!(zones, 5, "New York, London, Lord Howe, Kolkata and UTC");
    r.finish();
}

/// The series `scripts/fixtures.mjs` gives LTTB: from SplitMix64, x steps
/// of 0.5 to 1.5, y a walk or spikes over noise.
fn lttb_series(kind: &str, seed: u64, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut r = SplitMix64::new(seed);
    let (mut xs, mut ys) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut x, mut y) = (0.0, 0.0);
    for _ in 0..n {
        x += 0.5 + r.next_f64();
        if kind == "walk" {
            y += r.next_f64() - 0.5;
        } else {
            y = if r.next_f64() < 0.02 { 50.0 * r.next_f64() } else { r.next_f64() };
        }
        xs.push(x);
        ys.push(y);
    }
    (xs, ys)
}

/// REQ-SAMPLE-060, REQ-REF-010: LTTB keeps the points the reference
/// implementation (flot-downsample af872e0) keeps, index for index.
#[test]
fn lttb_matches_the_reference() {
    let text = fixture("lttb.txt");
    let mut r = Report::new("lttb");
    for line in cases(&text) {
        let (head, tail) = split(line);
        let (seed, n, threshold): (u64, usize, usize) = (head[1].parse().unwrap(), head[2].parse().unwrap(), head[3].parse().unwrap());
        let (xs, ys) = lttb_series(head[0], seed, n);
        let got = lttb(&xs, &ys, threshold);
        let want: Vec<usize> = tail.split(' ').filter(|w| !w.is_empty()).map(|w| w.parse().unwrap()).collect();
        r.check(got == want, || format!("{} {} {} {}\n  got {} indices, want {}", head[0], seed, n, threshold, got.len(), want.len()));
    }
    r.finish();
}
