//! The debug build's own parts (feature `debug`, REQ-WASM-060): a panic hook
//! that prints a panic's message and location with `console.error` before
//! the trap, and the `debug-panic` chart, which panics when framed so the
//! shell's trap handling can be tested (REQ-WASM-040). A release build has
//! neither. These are the only charts the wasm crate defines (REQ-ARCH-020).

use std::sync::atomic::{AtomicBool, Ordering};

use glyphplot::chart::{Accepted, Appended, CellPos, Changes, Charset, Chart as Plot, Env, Hit, MakeError, Meta, Mono, Size, Table};
use glyphplot::data::{Data, DataError, Shape};
use glyphplot::grid::Grid;
use glyphplot::input::Input;
use glyphplot::options::Options;
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
