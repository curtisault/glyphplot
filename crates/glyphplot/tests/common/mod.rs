//! What the contract's tests share: the charts under test (`CHART=`), the
//! fixtures a chart accepts, the font's ranges, the label text a chart may
//! draw from its data, panics caught as failures, a pool of threads, and the
//! tally each check adds to (REQ-CONTRACT-010, REQ-CONTRACT-020).

pub mod snap;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use glyphplot::chart::Env;
use glyphplot::data::{Data, Values};
use glyphplot::fixtures::{Fixture, FIXTURES};
use glyphplot::grid::Grid;
use glyphplot::registry::{self, Entry};

/// The play times every frame is drawn at (the plan's contract rules).
pub const TIMES: [f64; 4] = [0.0, 1.0, 2.5, 6.0];

/// The three environments: colour, one ink and paper.
pub fn envs() -> [(&'static str, Env<'static>); 3] {
    [("plain", Env::plain()), ("mono", Env { mono: true, ..Env::plain() }), ("paper", Env { paper: true, ..Env::plain() })]
}

/// Size set S: 1 × 1 (no panic only), `META.min`, 40 × 12, 80 × 24 and
/// 320 × 120, without repeats, in that order.
pub fn sizes(entry: &Entry) -> Vec<(usize, usize)> {
    let min = (usize::from(entry.meta.min.cols), usize::from(entry.meta.min.rows));
    let mut out: Vec<(usize, usize)> = Vec::new();
    for s in [(1, 1), min, (40, 12), (80, 24), (320, 120)] {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// The charts under test: every registered chart, or the types `CHART`
/// names, comma-separated. An unknown type fails the test
/// (REQ-CONTRACT-020).
pub fn charts() -> Vec<&'static Entry> {
    let all: Vec<&'static Entry> = registry::entries().collect();
    let wanted = std::env::var("CHART").unwrap_or_default();
    let names: Vec<&str> = wanted.split([',', ' ']).map(str::trim).filter(|s| !s.is_empty()).collect();
    if names.is_empty() {
        return all;
    }
    let unknown: Vec<&str> = names.iter().copied().filter(|n| !all.iter().any(|e| e.meta.slug == *n)).collect();
    assert!(
        unknown.is_empty(),
        "CHART names no registered chart: {}; the charts are {}",
        unknown.join(", "),
        all.iter().map(|e| e.meta.slug).collect::<Vec<_>>().join(", ")
    );
    all.into_iter().filter(|e| names.contains(&e.meta.slug)).collect()
}

/// The fixtures whose shape the chart accepts, in the plan's order.
pub fn accepted(entry: &Entry) -> Vec<&'static Fixture> {
    FIXTURES.iter().filter(|f| entry.meta.shapes.contains(&f.shape)).collect()
}

/// The options text of a mode: `{}` for the default, the first listed.
pub fn mode_options(entry: &Entry, mode: &str) -> String {
    if mode == entry.meta.modes[0] {
        "{}".to_string()
    } else {
        format!(r#"{{"mode":{}}}"#, serde_json::Value::from(mode))
    }
}

/// The font's code point ranges, read from `web/fonts/unicode-range.txt`,
/// the file the build writes beside the font (REQ-FONT-010).
pub struct Font(Vec<(u32, u32)>);

impl Font {
    /// The ranges, parsed once.
    pub fn get() -> &'static Font {
        static FONT: OnceLock<Font> = OnceLock::new();
        FONT.get_or_init(|| {
            let text = include_str!("../../../../web/fonts/unicode-range.txt");
            let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("U+"), 16).unwrap_or_else(|_| panic!("unicode-range.txt: {s:?}"));
            let ranges = text
                .split(',')
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(|r| match r.split_once('-') {
                    Some((a, b)) => (hex(a), hex(b)),
                    None => (hex(r), hex(r)),
                })
                .collect();
            Font(ranges)
        })
    }

    /// Whether the font draws `c`.
    pub fn contains(&self, c: u32) -> bool {
        self.0.iter().any(|&(a, b)| a <= c && c <= b)
    }
}

/// Every character of the label text that comes from the data or the
/// options: column names, category labels, string values and every string in
/// the options (REQ-CHAR-010). Only these may be drawn outside the font.
pub fn label_chars(data: &Data, options_json: &str) -> BTreeSet<char> {
    let mut out = BTreeSet::new();
    for (name, table) in data.tables() {
        out.extend(name.chars());
        for column in table.columns() {
            out.extend(column.name.chars());
            for label in &column.categories {
                out.extend(label.chars());
            }
            if let Values::Text(texts) = &column.values {
                for t in texts {
                    out.extend(t.chars());
                }
            }
        }
    }
    for (name, _) in data.matrices() {
        out.extend(name.chars());
    }
    fn strings(v: &serde_json::Value, out: &mut BTreeSet<char>) {
        match v {
            serde_json::Value::String(s) => out.extend(s.chars()),
            serde_json::Value::Array(a) => a.iter().for_each(|e| strings(e, out)),
            serde_json::Value::Object(m) => m.values().for_each(|e| strings(e, out)),
            _ => {}
        }
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(options_json) {
        strings(&v, &mut out);
    }
    out
}

thread_local! {
    /// Whether a panic on this thread is caught by [`guarded`].
    static QUIET: Cell<bool> = const { Cell::new(false) };
    /// The last caught panic's message and place.
    static CAUGHT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Runs `f`, turning a panic into its message and place instead of failing
/// the test, so one run reports every failure (rule 4). Panics outside
/// `guarded` print as usual.
pub fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    static HOOK: OnceLock<()> = OnceLock::new();
    HOOK.get_or_init(|| {
        let earlier = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if QUIET.with(Cell::get) {
                CAUGHT.with(|c| *c.borrow_mut() = Some(info.to_string().replace('\n', " ")));
            } else {
                earlier(info);
            }
        }));
    });
    let was = QUIET.with(|q| q.replace(true));
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    QUIET.with(|q| q.set(was));
    result.map_err(|_| CAUGHT.with(|c| c.borrow_mut().take()).unwrap_or_else(|| "a panic".to_string()))
}

/// Runs `work` on every job over as many threads as the machine has, the
/// longest jobs first as `cost` ranks them, and returns the results in the
/// jobs' order (REQ-CONTRACT-020).
pub fn parallel<J: Sync, R: Send>(jobs: &[J], cost: impl Fn(&J) -> usize, work: impl Fn(&J) -> R + Sync) -> Vec<R> {
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(cost(&jobs[i])));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(jobs.len()).max(1);
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..jobs.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(&i) = order.get(k) else { break };
                let r = work(&jobs[i]);
                results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    results.into_inner().unwrap_or_else(|e| e.into_inner()).into_iter().map(|r| r.expect("every job ran")).collect()
}

/// How many failures of one rule a tally keeps word for word; the rest are
/// counted.
const EXAMPLES: u64 = 6;

/// What one chart's checks found: checks and failures by rule, the first few
/// failures of each rule in words, frames drawn and time taken.
#[derive(Debug, Default)]
pub struct Tally {
    /// Checks made, by rule number.
    pub checks: BTreeMap<u32, u64>,
    /// Checks failed, by rule number.
    pub fails: BTreeMap<u32, u64>,
    /// The first failures of each rule, in words.
    pub examples: Vec<String>,
    /// Frames drawn.
    pub frames: u64,
    /// Time spent, summed over the threads that did the work.
    pub time: Duration,
}

impl Tally {
    /// Counts a check of `rule`, and a failure when `ok` is false, described
    /// by `what` only then.
    pub fn check(&mut self, rule: u32, ok: bool, what: impl FnOnce() -> String) {
        *self.checks.entry(rule).or_default() += 1;
        if !ok {
            let n = self.fails.entry(rule).or_default();
            *n += 1;
            if *n <= EXAMPLES {
                self.examples.push(format!("rule {rule}: {}", what()));
            }
        }
    }

    /// Adds another tally of the same chart.
    pub fn merge(&mut self, other: Tally) {
        for (r, n) in other.checks {
            *self.checks.entry(r).or_default() += n;
        }
        for (r, n) in other.fails {
            let had = *self.fails.get(&r).unwrap_or(&0);
            *self.fails.entry(r).or_default() += n;
            let keep = EXAMPLES.saturating_sub(had);
            let prefix = format!("rule {r}: ");
            self.examples.extend(other.examples.iter().filter(|e| e.starts_with(&prefix)).take(keep as usize).cloned());
        }
        self.frames += other.frames;
        self.time += other.time;
    }

    /// Whether every check passed.
    pub fn ok(&self) -> bool {
        self.fails.values().all(|&n| n == 0)
    }
}

/// Prints the per-chart table of `rules` (shown with `--nocapture`) and fails
/// with every failure of every chart in one message (REQ-CONTRACT-020).
pub fn report(title: &str, rules: &[u32], tallies: &[(&'static str, Tally)], wall: Duration) {
    let mut head = format!("{:<12} {:>8}", "chart", "frames");
    for r in rules {
        head += &format!(" {:>9}", format!("rule {r}"));
    }
    head += &format!(" {:>9}", "thread s");
    println!("\n{title}: {} chart(s), {:.2} s wall", tallies.len(), wall.as_secs_f64());
    println!("{head}");
    for (slug, t) in tallies {
        let mut line = format!("{slug:<12} {:>8}", t.frames);
        for r in rules {
            let cell = match (t.checks.get(r), t.fails.get(r)) {
                (None, _) => "-".to_string(),
                (Some(_), Some(&f)) if f > 0 => format!("{f} FAIL"),
                (Some(&n), _) => format!("{n} ok"),
            };
            line += &format!(" {cell:>9}");
        }
        line += &format!(" {:>9.2}", t.time.as_secs_f64());
        println!("{line}");
    }
    let failed: Vec<String> = tallies
        .iter()
        .filter(|(_, t)| !t.ok())
        .map(|(slug, t)| {
            let counts: Vec<String> = t.fails.iter().filter(|(_, &n)| n > 0).map(|(r, n)| format!("rule {r}: {n}")).collect();
            format!("{slug} fails {}:\n    {}", counts.join(", "), t.examples.join("\n    "))
        })
        .collect();
    assert!(failed.is_empty(), "{title}: {} chart(s) fail\n  {}", failed.len(), failed.join("\n  "));
}

/// Where two frames first differ, in words: the cell, its glyph, ink and
/// back in each; or their sizes.
pub fn first_difference(a: &Grid, b: &Grid) -> String {
    if (a.cols(), a.rows()) != (b.cols(), b.rows()) {
        return format!("{}×{} against {}×{}", a.cols(), a.rows(), b.cols(), b.rows());
    }
    let cell = |g: &Grid, i: usize| format!("U+{:04X} ink {} back {}", g.glyphs()[i], g.inks()[i], g.backs()[i]);
    let differ = (0..a.glyphs().len()).filter(|&i| a.glyphs()[i] != b.glyphs()[i] || a.inks()[i] != b.inks()[i] || a.backs()[i] != b.backs()[i]);
    let all: Vec<usize> = differ.collect();
    match all.first() {
        None => "no difference".to_string(),
        Some(&i) => format!("{} cell(s) differ, first at col {} row {}: {} against {}", all.len(), i % a.cols(), i / a.cols(), cell(a, i), cell(b, i)),
    }
}
