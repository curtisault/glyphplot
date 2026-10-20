//! A chart's options: a JSON object layered on the defaults its `META`
//! declares (REQ-OPT-010, `spec/schemas/options.schema.json`).
//!
//! [`Options::parse`] reads the overrides a page, the CLI or a test gives; an
//! empty string, or only whitespace, counts as no options, and anything but a
//! JSON object, invalid JSON or JSON of another kind, fails with `options are
//! not a JSON object: <message>` (REQ-REG-010, REQ-WASM-020). [`Options::layered`] puts them over a chart's defaults key
//! by key: a key present in the overrides wins, every other default fills
//! in. The typed getters fall back to the default on a missing key or a
//! wrong type, a list with one wrong element falling back whole, and record
//! every key they read, so the contract can check that each has a default
//! ([`Options::read`]). The record is shared by the options a caller made and
//! every copy layered from them, so a caller that hands options to a chart
//! (`registry::make_with`, `Chart::set_options`) reads back what the chart
//! read.

use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};

use crate::chart::{MakeError, Meta};

/// A chart's options: overrides layered on defaults (REQ-OPT-010).
#[derive(Clone, Debug, Default)]
pub struct Options {
    overrides: Map<String, Value>,
    defaults: Map<String, Value>,
    read: Arc<Mutex<Vec<String>>>,
}

impl PartialEq for Options {
    /// Equal when the overrides and the defaults are; what was read does not
    /// count.
    fn eq(&self, other: &Options) -> bool {
        self.overrides == other.overrides && self.defaults == other.defaults
    }
}

/// The JSON object a text holds, `None` for empty or blank text, or what is
/// wrong with any other text: the parser's message for text that is not JSON,
/// the kind of value for JSON that is not an object.
fn object(json: &str) -> Result<Option<Map<String, Value>>, String> {
    if json.trim().is_empty() {
        return Ok(None);
    }
    match serde_json::from_str::<Value>(json) {
        Ok(Value::Object(map)) => Ok(Some(map)),
        Ok(Value::Array(_)) => Err("an array".into()),
        Ok(Value::Number(_)) => Err("a number".into()),
        Ok(Value::String(_)) => Err("a string".into()),
        Ok(Value::Bool(_)) => Err("a boolean".into()),
        Ok(Value::Null) => Err("null".into()),
        Err(e) => Err(e.to_string()),
    }
}

impl Options {
    /// No options.
    pub fn new() -> Options {
        Options::default()
    }

    /// Overrides from their JSON text (REQ-OPT-010): an object's keys; an
    /// empty or blank string is no options; invalid JSON, or JSON that is not
    /// an object, is [`MakeError::Options`] (REQ-WASM-020).
    pub fn parse(json: &str) -> Result<Options, MakeError> {
        let overrides = object(json).map_err(|message| MakeError::Options { message })?.unwrap_or_default();
        Ok(Options { overrides, ..Options::default() })
    }

    /// Overrides from a JSON value: an object's keys, anything else none.
    pub fn from_value(value: Value) -> Options {
        match value {
            Value::Object(overrides) => Options { overrides, ..Options::default() },
            _ => Options::default(),
        }
    }

    /// These options' overrides layered on `defaults`, a JSON object's text
    /// as `META.options` holds it (REQ-OPT-010). A key present in the
    /// overrides wins; every other default fills in. The defaults these
    /// options already had are replaced. The copy shares these options'
    /// record of keys read.
    pub fn layered(&self, defaults: Option<&str>) -> Options {
        let defaults = defaults.and_then(|d| object(d).ok().flatten()).unwrap_or_default();
        Options { overrides: self.overrides.clone(), defaults, read: Arc::clone(&self.read) }
    }

    /// These options layered on a chart's `META.options`.
    pub fn for_meta(&self, meta: &Meta) -> Options {
        self.layered(meta.options)
    }

    /// Records that `key` was read.
    fn note(&self, key: &str) {
        let mut read = self.read.lock().unwrap_or_else(|e| e.into_inner());
        if !read.iter().any(|k| k == key) {
            read.push(key.to_string());
        }
    }

    /// The value of `key`: the override when given, else the default; `None`
    /// when neither has it. The key is recorded as read.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.note(key);
        self.overrides.get(key).or_else(|| self.defaults.get(key))
    }

    /// The override of `key` when `take` accepts it, else the default's when
    /// it accepts that, else `None`. The key is recorded as read.
    fn typed<'a, T>(&'a self, key: &str, take: impl Fn(&'a Value) -> Option<T>) -> Option<T> {
        self.note(key);
        self.overrides.get(key).and_then(&take).or_else(|| self.defaults.get(key).and_then(&take))
    }

    /// `key` as a string, else `""`.
    pub fn str(&self, key: &str) -> &str {
        self.typed(key, Value::as_str).unwrap_or("")
    }

    /// `key` as a boolean, else false.
    pub fn bool(&self, key: &str) -> bool {
        self.typed(key, Value::as_bool).unwrap_or(false)
    }

    /// `key` as a finite number, else NaN.
    pub fn f64(&self, key: &str) -> f64 {
        self.typed(key, |v| v.as_f64().filter(|x| x.is_finite())).unwrap_or(f64::NAN)
    }

    /// `key` as a list of strings, else empty; a list with one element that
    /// is not a string falls back whole.
    pub fn strs(&self, key: &str) -> Vec<String> {
        self.typed(key, |v| v.as_array().and_then(|a| a.iter().map(|e| e.as_str().map(str::to_string)).collect::<Option<Vec<_>>>())).unwrap_or_default()
    }

    /// `key` as an object, else `None`.
    pub fn object(&self, key: &str) -> Option<&Map<String, Value>> {
        self.typed(key, Value::as_object)
    }

    /// `key` as one of `choices`, else the default's when it is one, else the
    /// first choice.
    pub fn choice(&self, key: &str, choices: &[&'static str]) -> &'static str {
        self.typed(key, |v| v.as_str().and_then(|s| choices.iter().copied().find(|c| *c == s))).unwrap_or(choices[0])
    }

    /// Every key read so far, by these options or a copy layered from them,
    /// in the order first read (REQ-OPT-010).
    pub fn read(&self) -> Vec<String> {
        self.read.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Whether `key` has a default.
    pub fn has_default(&self, key: &str) -> bool {
        self.defaults.contains_key(key)
    }

    /// The keys read that have no default: empty for a chart that keeps
    /// REQ-OPT-010.
    pub fn read_without_default(&self) -> Vec<String> {
        self.read().into_iter().filter(|k| !self.has_default(k)).collect()
    }

    /// The overrides alone, as JSON.
    pub fn overrides_json(&self) -> String {
        Value::Object(self.overrides.clone()).to_string()
    }

    /// The options as a chart reads them, defaults filled in, as JSON.
    pub fn to_json(&self) -> String {
        let mut all = self.defaults.clone();
        all.extend(self.overrides.clone());
        Value::Object(all).to_string()
    }

    /// The `mode` option checked against `meta.modes`: the default, the
    /// first mode, when none is given; a mode not listed is
    /// [`MakeError::Mode`], naming the valid ones (REQ-CHART-030).
    pub fn mode(&self, meta: &Meta) -> Result<&'static str, MakeError> {
        self.note("mode");
        let given = self.overrides.get("mode").or_else(|| self.defaults.get("mode"));
        match given {
            None => Ok(meta.modes[0]),
            Some(Value::String(s)) => meta.modes.iter().copied().find(|m| m == s).ok_or_else(|| MakeError::Mode { mode: s.clone(), modes: meta.modes }),
            Some(other) => Err(MakeError::Mode { mode: other.to_string(), modes: meta.modes }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULTS: &str = r#"{"mode":"line","spanGaps":false,"title":"","encode":{},"tags":["a","b"],"baseline":0}"#;

    /// REQ-OPT-010: overrides win key by key, defaults fill in, wrong types
    /// fall back to the default, a list falls back whole, and every key read
    /// is recorded.
    #[test]
    fn getters_fall_back() {
        let o = Options::parse(r#"{"spanGaps":"yes","title":"Prices","tags":["x",1],"baseline":2.5,"extra":true}"#).unwrap().layered(Some(DEFAULTS));
        assert!(!o.bool("spanGaps"), "a string is not a boolean: the default");
        assert_eq!(o.str("title"), "Prices");
        assert_eq!(o.strs("tags"), ["a", "b"], "one wrong element: the whole default");
        assert_eq!(o.f64("baseline"), 2.5);
        assert_eq!(o.str("mode"), "line");
        assert!(o.f64("missing").is_nan());
        assert_eq!(o.read(), ["spanGaps", "title", "tags", "baseline", "mode", "missing"]);
        assert_eq!(o.read_without_default(), ["missing"]);
        assert!(o.to_json().contains(r#""extra":true"#));
    }

    /// REQ-OPT-010, REQ-REG-010, REQ-WASM-020: no options from empty or
    /// blank text; an error for invalid JSON and for JSON that is not an
    /// object, naming what it is.
    #[test]
    fn only_an_object_or_nothing_parses() {
        for none in ["", "  ", "\n\t"] {
            assert_eq!(Options::parse(none).unwrap(), Options::new(), "{none:?}");
        }
        let e = Options::parse("{\"mode\":").unwrap_err().to_string();
        assert!(e.starts_with("options are not a JSON object: "), "{e}");
        for (bad, kind) in [("[1,2]", "an array"), ("null", "null"), ("3", "a number"), ("\"line\"", "a string"), (" true ", "a boolean")] {
            assert_eq!(Options::parse(bad).unwrap_err().to_string(), format!("options are not a JSON object: {kind}"), "{bad:?}");
        }
    }
}
