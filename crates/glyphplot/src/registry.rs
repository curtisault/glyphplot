//! The chart registry: every family this build holds, in registry order, and
//! the types each plays (REQ-REG-010, ADR-125). One family in a family's
//! wasm module, every one with the feature `all`.
//!
//! Each family's `mod.rs` holds a hand-written `REGISTRY` of its types, and
//! `charts/mod.rs` lists the families behind their features
//! ([`FAMILIES`]). [`make`] makes a fresh chart of a type by its name with
//! options layered on its defaults, failing with `there is no chart named
//! "<type>"` or `options are not a JSON object: <message>` (REQ-REG-010,
//! REQ-WASM-020); it keeps no state, so a failure leaves nothing behind.

use crate::chart::{Chart, Make, MakeError, Meta};
use crate::options::Options;
use crate::Budget;

pub use crate::charts::FAMILIES;

/// One chart type: its meta and its maker (REQ-CHART-010).
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// What the type says about itself.
    pub meta: &'static Meta,
    /// A fresh chart of the type.
    pub make: Make,
}

/// A chart family: one Cargo feature and one wasm module, `web/pkg/<name>/`.
#[derive(Clone, Copy, Debug)]
pub struct Family {
    /// The feature's and the module's name.
    pub name: &'static str,
    /// What the family's module may cost.
    pub budget: Budget,
    /// The types it plays, in registry order.
    pub charts: &'static [Entry],
}

/// The names no chart type may take (REQ-REG-020).
pub const RESERVED: [&str; 3] = ["render", "demo", "list"];

/// Every type compiled in, in registry order.
pub fn entries() -> impl Iterator<Item = &'static Entry> {
    FAMILIES.iter().flat_map(|f| f.charts.iter())
}

/// Every type's meta compiled in, in registry order: `charts_json()` and the
/// manifest's `charts` (REQ-MANIFEST-010).
pub fn metas() -> impl Iterator<Item = &'static Meta> {
    entries().map(|e| e.meta)
}

/// The meta of the type named `slug`.
pub fn meta(slug: &str) -> Option<&'static Meta> {
    metas().find(|m| m.slug == slug)
}

/// Every type's meta as one JSON array, for the module's `charts_json()`.
pub fn charts_json() -> String {
    serde_json::to_string(&metas().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into())
}

/// A fresh chart of the type named `slug`, its options the JSON object
/// `options_json` layered on its defaults (REQ-REG-010): an empty text or
/// JSON that is not an object is no options. Fails with `there is no chart
/// named "<type>"`, `options are not a JSON object: <message>`, or an error
/// naming the valid modes (REQ-CHART-030).
pub fn make(slug: &str, options_json: &str) -> Result<(&'static Meta, Box<dyn Chart>), MakeError> {
    let entry = entries().find(|e| e.meta.slug == slug).ok_or_else(|| MakeError::NoChart { name: slug.to_string() })?;
    let options = Options::parse(options_json)?;
    make_with(entry.meta.slug, &options)
}

/// As [`make`], from options already parsed: the chart's options are
/// `options` layered on its defaults, sharing `options`' record of keys read,
/// so the caller can check after frames and inputs that every key the chart
/// read has a default (REQ-OPT-010, contract rule 2).
pub fn make_with(slug: &str, options: &Options) -> Result<(&'static Meta, Box<dyn Chart>), MakeError> {
    let entry = entries().find(|e| e.meta.slug == slug).ok_or_else(|| MakeError::NoChart { name: slug.to_string() })?;
    let chart = (entry.make)(&options.for_meta(entry.meta))?;
    Ok((entry.meta, chart))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module name as `spec/schemas/budgets.schema.json` allows it: lowercase
    /// words of letters and digits joined by single hyphens.
    fn is_module_name(name: &str) -> bool {
        !name.is_empty() && name.split('-').all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
    }

    #[test]
    fn names_are_module_names_and_unique() {
        for (i, family) in FAMILIES.iter().enumerate() {
            assert!(is_module_name(family.name), "{:?} is not a module name", family.name);
            assert!(FAMILIES[..i].iter().all(|f| f.name != family.name), "{:?} is registered twice", family.name);
            for e in family.charts {
                assert_eq!(e.meta.family, family.name, "{} is listed under its own family", e.meta.slug);
            }
        }
        let slugs: Vec<&str> = metas().map(|m| m.slug).collect();
        for (i, s) in slugs.iter().enumerate() {
            assert!(is_module_name(s) && !slugs[..i].contains(s), "{s}");
        }
    }

    #[test]
    fn module_names() {
        assert!(is_module_name("line") && is_module_name("sky-2"));
        assert!(!is_module_name("") && !is_module_name("Line") && !is_module_name("a--b") && !is_module_name("-a") && !is_module_name("a_b"));
    }

    /// REQ-REG-020: no type is named render, demo or list.
    #[test]
    fn no_reserved_names() {
        for m in metas() {
            assert!(!RESERVED.contains(&m.slug), "{} is reserved", m.slug);
        }
    }

    #[cfg(feature = "all")]
    #[test]
    fn all_holds_every_family() {
        let names: Vec<&str> = FAMILIES.iter().map(|f| f.name).collect();
        assert_eq!(names, ["line"]);
        assert_eq!(metas().map(|m| m.slug).collect::<Vec<_>>(), ["line"]);
    }

    /// REQ-REG-010, REQ-WASM-020, REQ-CHART-030: making fails with the plan's
    /// messages, and a failure leaves the registry working.
    #[cfg(feature = "line")]
    #[test]
    fn make_errors_name_the_problem() {
        assert_eq!(make("pie", "").err().unwrap().to_string(), "there is no chart named \"pie\"");
        let e = make("line", "{nope").err().unwrap().to_string();
        assert!(e.starts_with("options are not a JSON object: "), "{e}");
        assert_eq!(make("line", r#"{"mode":"zigzag"}"#).err().unwrap().to_string(), "there is no mode \"zigzag\": the modes are line, step, area");
        let (meta, _) = make("line", r#"{"mode":"area"}"#).unwrap();
        assert_eq!(meta.slug, "line");
        assert!(make("line", "[1]").is_ok(), "not an object: no options");
        let v: serde_json::Value = serde_json::from_str(&charts_json()).unwrap();
        assert_eq!(v[0]["slug"], "line");
    }

    /// REQ-OPT-010, contract rule 2: options handed to a chart record what it
    /// read, and every key read has a default in its meta.
    #[cfg(feature = "line")]
    #[test]
    fn options_record_what_the_chart_reads() {
        use crate::chart::Env;
        use crate::grid::Grid;
        let options = Options::parse(r#"{"mode":"area"}"#).unwrap();
        let (meta, mut c) = make_with("line", &options).unwrap();
        c.set_data(crate::fixtures::generate("multi-4").unwrap()).unwrap();
        c.frame(0.0, &Env::plain(), &mut Grid::new(80, 24));
        let defaults: serde_json::Value = serde_json::from_str(meta.options.unwrap()).unwrap();
        let read = options.read();
        assert!(read.len() >= 10, "{read:?}");
        for key in &read {
            assert!(defaults.get(key).is_some(), "`{key}` has no default");
        }
    }

    /// REQ-OPT-020: new options keep the data and the view; a bad mode keeps
    /// the old options.
    #[cfg(feature = "line")]
    #[test]
    fn set_options_keeps_data_and_view() {
        use crate::chart::Env;
        use crate::grid::Grid;
        use crate::view::ViewPatch;
        let (_, mut c) = make("line", "").unwrap();
        c.set_data(crate::fixtures::generate("sine-1k").unwrap()).unwrap();
        c.set_view(&ViewPatch::from_json(r#"{"x":{"domain":[100,400]},"hidden":["nothing"]}"#).unwrap());
        let view = c.view().clone();
        let mut a = Grid::new(60, 16);
        c.frame(0.0, &Env::plain(), &mut a);
        c.set_options(&Options::parse(r#"{"mode":"area"}"#).unwrap()).unwrap();
        assert_eq!(c.view(), &view);
        let mut b = Grid::new(60, 16);
        c.frame(0.0, &Env::plain(), &mut b);
        assert_ne!(a, b, "the new mode draws");
        let (_, mut fresh) = make("line", r#"{"mode":"area"}"#).unwrap();
        fresh.set_data(crate::fixtures::generate("sine-1k").unwrap()).unwrap();
        fresh.set_view(&ViewPatch::whole(&view));
        let mut f = Grid::new(60, 16);
        fresh.frame(0.0, &Env::plain(), &mut f);
        assert_eq!(b, f, "the data was kept");
        assert!(c.set_options(&Options::parse(r#"{"mode":"pie"}"#).unwrap()).is_err());
        c.frame(0.0, &Env::plain(), &mut a);
        assert_eq!(a, b, "a refused mode keeps the options");
    }
}
