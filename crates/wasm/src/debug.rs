//! The debug build's own parts (feature `debug`, REQ-WASM-060): a panic hook
//! that prints a panic's message and location with `console.error` before
//! the trap, and the `debug-panic` chart, which panics when framed so the
//! shell's trap handling can be tested (REQ-WASM-040). A release build has
//! neither. These are the only charts the wasm crate defines (REQ-ARCH-020).
//!
//! It also reaches the core's ports and fixtures from JavaScript, so the
//! reference tests run in wasm as well as natively (REQ-REF-010,
//! REQ-TICK-010, REQ-FIX-010; web/test/reference.test.mjs):
//! [`debug_reference`] answers one case line of a capture file in
//! `crates/glyphplot/tests/fixtures/`, reading it as
//! `crates/glyphplot/tests/reference.rs` reads it, and
//! [`debug_fixture_digest`] gives a fixture's digest.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use glyphplot::chart::{Accepted, Appended, CellPos, Changes, Charset, Chart as Plot, Env, Hit, MakeError, Meta, Mono, Size, Table};
use glyphplot::data::{Data, DataError, Shape};
use glyphplot::downsample::lttb;
use glyphplot::fixtures::SplitMix64;
use glyphplot::format::{js, tick_format, Locale, NumberFormat, Specifier};
use glyphplot::grid::Grid;
use glyphplot::input::Input;
use glyphplot::locale::ZoneTable;
use glyphplot::options::Options;
use glyphplot::scale::{log_nice, nice};
use glyphplot::ticks::{log_ticks, tick_increment, tick_step, ticks, LogTickFormat};
use glyphplot::time::{strftime, utc_nice, utc_tick_label, utc_ticks, Zone};
use glyphplot::view::{View, ViewPatch};
use glyphplot::Budget;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// `console.error`.
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
    /// `console.warn`.
    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn console_warn(message: &str);
}

/// Whether the hook is in.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Installs the panic hook once: it prints `glyphplot: panicked at
/// <file>:<line>:<col>:` and the message with `console.error`; the trap
/// follows (`panic = "abort"`). It allocates only the one string it prints.
pub fn install() {
    if !INSTALLED.swap(true, Ordering::Relaxed) {
        std::panic::set_hook(Box::new(|info| console_error(&format!("glyphplot: {info}"))));
    }
}

/// Logs `message` with `console.warn`.
pub fn warn(message: &str) {
    console_warn(message);
}

/// The `debug-panic` type.
pub static PANIC_META: Meta = Meta {
    slug: "debug-panic",
    name: "debug panic",
    note: "panics when framed: the debug build's trap test",
    family: "all",
    modes: &["panic"],
    shapes: &[Shape::Series],
    options: None,
    min: Size { cols: 1, rows: 1 },
    drop: &[],
    mono: Mono::Auto,
    charset: Charset::Basic,
    budget: Budget::DEFAULT,
};

/// The debug charts' metas, in order.
const METAS: [&Meta; 1] = [&PANIC_META];

/// A debug chart of type `kind`, or `None` when `kind` is not one; options
/// are parsed as the registry parses them.
pub fn make(kind: &str, options_json: &str) -> Option<crate::Made> {
    (kind == PANIC_META.slug).then(|| Options::parse(options_json).map(|_| (&PANIC_META, Box::new(PanicChart::default()) as Box<dyn Plot>)))
}

/// `charts_json` with the debug charts' metas after the registry's.
pub fn with_metas(registry_json: String) -> String {
    let own: Vec<String> = METAS.iter().map(|m| m.to_json()).collect();
    match registry_json.strip_suffix(']') {
        Some("[") => format!("[{}]", own.join(",")),
        Some(head) => format!("{head},{}]", own.join(",")),
        None => registry_json,
    }
}

/// A chart that takes every call and panics when framed.
#[derive(Default)]
struct PanicChart {
    view: View,
    version: u64,
}

impl Plot for PanicChart {
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        self.version += 1;
        Ok(Accepted { rows: data.first_table().map_or(0, |t| t.len()), version: self.version, warnings: Vec::new() })
    }

    fn append(&mut self, batch: Data) -> Result<Appended, DataError> {
        let a = self.set_data(batch)?;
        Ok(Appended { rows: a.rows, dropped: 0, late: 0, version: a.version })
    }

    fn set_options(&mut self, _: &Options) -> Result<(), MakeError> {
        Ok(())
    }

    fn view(&self) -> &View {
        &self.view
    }

    fn set_view(&mut self, patch: &ViewPatch) {
        self.view.apply(patch);
    }

    fn input(&mut self, _: &Input) -> Changes {
        Changes::default()
    }

    fn frame(&mut self, t: f64, _: &Env, grid: &mut Grid) {
        panic!("debug-panic: framed at t = {t} on a {} × {} grid, as it always panics (REQ-WASM-060)", grid.cols(), grid.rows());
    }

    fn hit(&self, _: CellPos) -> Option<Hit> {
        None
    }

    fn table(&self, _: usize) -> Table {
        Table { caption: "debug panic".into(), summary: "debug panic chart".into(), ..Table::default() }
    }
}

/// A number of a case line, as `String(x)` wrote it.
fn num(s: &str) -> Result<f64, String> {
    s.parse().map_err(|_| format!("not a number: {s:?}"))
}

/// The numbers written as the capture files write them: JavaScript's
/// `String(x)` (`NaN`, `Infinity`), `-0` as `-0`, separated by spaces.
fn numbers(xs: &[f64]) -> String {
    let mut out = String::new();
    for (i, &x) in xs.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match x {
            _ if x.is_nan() => out.push_str("NaN"),
            f64::INFINITY => out.push_str("Infinity"),
            f64::NEG_INFINITY => out.push_str("-Infinity"),
            _ if x == 0.0 && x.is_sign_negative() => out.push_str("-0"),
            _ => js::to_string(x, &mut out),
        }
    }
    out
}

/// The head of a `head : tail` case line, split at spaces; the tail is the
/// expected answer, not read here.
fn head(line: &str) -> Vec<&str> {
    line.split_once(" : ").map_or(line.trim_end_matches(" :"), |(h, _)| h).split(' ').collect()
}

/// The `i`th field of a case, or an error naming it.
fn field<'a>(f: &[&'a str], i: usize, line: &str) -> Result<&'a str, String> {
    f.get(i).copied().ok_or_else(|| format!("{line:?} has no field {i}"))
}

thread_local! {
    /// The zone of the `zone-ticks.txt` block being answered: its `zone`
    /// line sets it, and the `ticks`, `nice` and `label` lines after it read
    /// it, as `tests/reference.rs` reads the file top to bottom.
    static ZONE: RefCell<Option<Zone>> = const { RefCell::new(None) };
}

/// The series `scripts/fixtures.mjs` gives LTTB, as `tests/reference.rs`
/// makes it: from SplitMix64, x steps of 0.5 to 1.5, y a walk or spikes over
/// noise.
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

/// A whole number of a case line.
fn whole(s: &str) -> Result<usize, String> {
    s.parse().map_err(|_| format!("not a whole number: {s:?}"))
}

/// The core's answer to one case line of `file`, written as the file
/// writes its expected part: the numbers after ` : ` for `ticks.txt`,
/// `nice.txt`, `time-ticks.txt`, the `ticks` and `nice` lines of `log.txt`
/// and `zone-ticks.txt`, and `lttb.txt` (the indices kept); the
/// tab-separated labels after the arguments for `format.txt`,
/// `time-format.txt`, the `format` lines of `log.txt` and the `label` lines
/// of `zone-ticks.txt`. A `zone` line of `zone-ticks.txt` sets the zone the
/// lines after it are answered in and answers with the zone's name.
fn answer(file: &str, line: &str) -> Result<String, String> {
    let locale = Locale::default();
    match file {
        "log.txt" => {
            if let Some(rest) = line.strip_prefix("format\t") {
                let f: Vec<&str> = rest.split('\t').collect();
                let arg = |i: usize| field(&f, i, line);
                let (base, a, b, n) = (num(arg(0)?)?, num(arg(1)?)?, num(arg(2)?)?, num(arg(3)?)?);
                let spec = match arg(4)? {
                    "auto" => None,
                    s => Some(Specifier::parse("format", s).map_err(|e| e.to_string())?),
                };
                let format = LogTickFormat::new([a, b], base, n, spec.as_ref(), &locale);
                return Ok(log_ticks(a, b, n, base).into_iter().map(|t| format.format(t)).collect::<Vec<_>>().join("\t"));
            }
            let h = head(line);
            let (base, a, b) = (num(field(&h, 1, line)?)?, num(field(&h, 2, line)?)?, num(field(&h, 3, line)?)?);
            match h[0] {
                "ticks" => Ok(numbers(&log_ticks(a, b, num(field(&h, 4, line)?)?, base))),
                "nice" => Ok(numbers(&log_nice(a, b, base))),
                other => Err(format!("unknown case {other:?} in {file}")),
            }
        }
        "zone-ticks.txt" => {
            if let Some(rest) = line.strip_prefix("zone ") {
                let (_, json) = rest.split_once(' ').ok_or_else(|| format!("{line:?} is not `zone name table`"))?;
                let table = ZoneTable::from_json(json).map_err(|e| e.0)?;
                let name = table.name.clone();
                ZONE.with(|z| *z.borrow_mut() = Some(Zone::table(table)));
                return Ok(name);
            }
            let zone = ZONE.with(|z| z.borrow().clone()).ok_or_else(|| "a `zone` line comes first in zone-ticks.txt".to_string())?;
            if let Some(rest) = line.strip_prefix("label\t") {
                let t = rest.split('\t').next().unwrap_or_default();
                let mut out = String::new();
                zone.tick_label(num(t)?, &locale.time, &mut out);
                return Ok(out);
            }
            let h = head(line);
            let (a, b, n) = (num(field(&h, 1, line)?)?, num(field(&h, 2, line)?)?, num(field(&h, 3, line)?)?);
            match h[0] {
                "ticks" => Ok(numbers(&zone.ticks(a, b, n))),
                "nice" => Ok(numbers(&zone.nice(a, b, n))),
                other => Err(format!("unknown case {other:?} in {file}")),
            }
        }
        "lttb.txt" => {
            let h = head(line);
            let kind = field(&h, 0, line)?;
            if kind != "walk" && kind != "spike" {
                return Err(format!("unknown case {kind:?} in {file}"));
            }
            let seed = whole(field(&h, 1, line)?)? as u64;
            let (n, threshold) = (whole(field(&h, 2, line)?)?, whole(field(&h, 3, line)?)?);
            let (xs, ys) = lttb_series(kind, seed, n);
            Ok(lttb(&xs, &ys, threshold).into_iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" "))
        }
        "ticks.txt" | "nice.txt" => {
            let h = head(line);
            let (a, b, n) = (num(field(&h, 1, line)?)?, num(field(&h, 2, line)?)?, num(field(&h, 3, line)?)?);
            Ok(match (file, h[0]) {
                ("ticks.txt", "ticks") => numbers(&ticks(a, b, n)),
                ("ticks.txt", "step") => numbers(&[tick_increment(a, b, n), tick_step(a, b, n)]),
                ("nice.txt", "linear") => numbers(&nice(a, b, n)),
                ("nice.txt", "utc") => numbers(&utc_nice(a, b, n)),
                (_, other) => return Err(format!("unknown case {other:?} in {file}")),
            })
        }
        "time-ticks.txt" => {
            let h = head(line);
            Ok(numbers(&utc_ticks(num(field(&h, 0, line)?)?, num(field(&h, 1, line)?)?, num(field(&h, 2, line)?)?)))
        }
        "format.txt" | "time-format.txt" => {
            let f: Vec<&str> = line.split('\t').collect();
            let mut out = String::new();
            let arg = |i: usize| field(&f, i, line);
            match (file, f[0]) {
                ("format.txt", "fixed") => js::to_fixed(num(arg(1)?)?, arg(2)?.parse().map_err(|_| "digits".to_string())?, &mut out),
                ("format.txt", "exponential") => js::to_exponential(num(arg(1)?)?, arg(2)?.parse().ok(), &mut out),
                ("format.txt", "precision") => js::to_precision(num(arg(1)?)?, arg(2)?.parse().map_err(|_| "precision".to_string())?, &mut out),
                ("format.txt", "string") => js::to_string(num(arg(1)?)?, &mut out),
                ("format.txt", "format") => {
                    let spec = Specifier::parse("format", arg(1)?).map_err(|e| e.to_string())?;
                    NumberFormat::new(&spec, &locale).format_into(num(arg(2)?)?, &mut out);
                }
                ("format.txt", "tick") => {
                    let (a, b, n) = (num(arg(1)?)?, num(arg(2)?)?, num(arg(3)?)?);
                    let spec = Specifier::parse("format", arg(4)?).map_err(|e| e.to_string())?;
                    let format = tick_format(a, b, n, &spec, &locale);
                    return Ok(ticks(a, b, n).into_iter().map(|t| format.format(t)).collect::<Vec<_>>().join("\t"));
                }
                ("time-format.txt", "label") => utc_tick_label(num(arg(1)?)?, &locale.time, &mut out),
                ("time-format.txt", "strftime") => strftime(arg(1)?, num(arg(2)?)?, &locale.time, &mut out),
                (_, other) => return Err(format!("unknown case {other:?} in {file}")),
            }
            Ok(out)
        }
        other => Err(format!("no capture file {other:?}")),
    }
}

/// Debug builds only: the core's answer to one case line of the capture
/// file `file` in `crates/glyphplot/tests/fixtures/`, written as the file
/// writes its expected part (REQ-REF-010, REQ-TICK-010). Throws for a file
/// or case it does not know.
#[wasm_bindgen]
pub fn debug_reference(file: &str, line: &str) -> Result<String, JsError> {
    answer(file, line).map_err(|e| JsError::new(&e))
}

/// Debug builds only: the digest of the fixture `name` generated in this
/// module, as `{:#018x}` of `glyphplot::fixtures::digest` (REQ-FIX-010).
/// Throws for a name that is not a fixture.
#[wasm_bindgen]
pub fn debug_fixture_digest(name: &str) -> Result<String, JsError> {
    let data = glyphplot::fixtures::generate(name).ok_or_else(|| JsError::new(&format!("there is no fixture named {name:?}")))?;
    Ok(format!("{:#018x}", glyphplot::fixtures::digest(&data)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every case of every capture file answers as the file writes it, text
    /// for text, natively; web/test/reference.test.mjs makes the same
    /// comparison in wasm (REQ-REF-010).
    #[test]
    fn answers_every_capture_line() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../glyphplot/tests/fixtures/");
        for file in ["ticks.txt", "nice.txt", "time-ticks.txt", "format.txt", "time-format.txt", "log.txt", "zone-ticks.txt", "lttb.txt"] {
            let text = std::fs::read_to_string(format!("{dir}{file}")).unwrap();
            let mut n = 0;
            for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
                let got = answer(file, line).unwrap();
                let want = if let Some(rest) = line.strip_prefix("zone ") {
                    rest.split(' ').next().unwrap_or_default().to_string()
                } else if file.ends_with("format.txt") || line.contains('\t') {
                    let f: Vec<&str> = line.split('\t').collect();
                    let at = match (file, f[0]) {
                        (_, "string" | "label") => 2,
                        (_, "tick") => 5,
                        ("log.txt", "format") => 6,
                        _ => 3,
                    };
                    f[at..].join("\t")
                } else {
                    line.split_once(" :").map_or("", |(_, t)| t).trim().to_string()
                };
                assert_eq!(got, want, "{file}: {line}");
                n += 1;
            }
            assert!(n > 0, "{file}");
        }
    }
}
