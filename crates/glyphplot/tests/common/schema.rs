//! A JSON Schema (draft 2020-12) validator for the subset of keywords
//! `spec/schemas/events.schema.json` and `view.schema.json` use, so the
//! contract checks every event and view against the schemas themselves
//! (rule 18, rule 16) with no new crate.
//!
//! It reads the schema files as data and applies each keyword's rule to the
//! instance: `type`, `enum`, `const`, `required`, `properties`,
//! `additionalProperties`, `prefixItems`, `items`, `minItems`, `maxItems`,
//! `uniqueItems`, `minimum`, `exclusiveMinimum`, `minLength`, `pattern` (the
//! one pattern the schemas use), `$ref` (within a file and to a sibling
//! file), `allOf`, `anyOf`, `oneOf` and `if`/`then`/`else`. Annotations
//! (`description`, `title`, `$schema`, `$id`, `default`, `examples`) are
//! ignored. Any other keyword is an error, so a schema growing a rule this
//! validator does not know fails the contract instead of passing unchecked.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

/// The schema files the validator knows, by file name.
pub struct Schemas(BTreeMap<&'static str, Value>);

/// Keywords that only annotate.
const ANNOTATIONS: [&str; 6] = ["description", "title", "$schema", "$id", "default", "examples"];

/// The only `pattern` the schemas use: a kebab-case chart type.
const KEBAB: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

impl Schemas {
    /// The events and view schemas, read once from `spec/schemas/`.
    pub fn get() -> &'static Schemas {
        static SCHEMAS: OnceLock<Schemas> = OnceLock::new();
        SCHEMAS.get_or_init(|| {
            let parse = |name: &str, text: &str| serde_json::from_str(text).unwrap_or_else(|e| panic!("spec/schemas/{name}: {e}"));
            let mut files = BTreeMap::new();
            files.insert("events.schema.json", parse("events.schema.json", include_str!("../../../../spec/schemas/events.schema.json")));
            files.insert("view.schema.json", parse("view.schema.json", include_str!("../../../../spec/schemas/view.schema.json")));
            Schemas(files)
        })
    }

    /// Every way `instance` breaks the schema of file `file`, each with the
    /// instance path; empty when it is valid.
    pub fn validate(&self, file: &'static str, instance: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        match self.0.get(file) {
            Some(schema) => self.check(file, schema, instance, "$", &mut errors),
            None => errors.push(format!("no schema file {file}")),
        }
        errors
    }

    /// The schema a `$ref` names, from `file`, and the file it is in.
    fn resolve(&self, file: &'static str, reference: &str) -> Result<(&'static str, &Value), String> {
        let (target, pointer) = reference.split_once('#').unwrap_or((reference, ""));
        let target: &'static str = if target.is_empty() {
            file
        } else {
            self.0.keys().copied().find(|k| *k == target).ok_or_else(|| format!("$ref to an unknown file {reference}"))?
        };
        let root = &self.0[target];
        let found = if pointer.is_empty() { Some(root) } else { root.pointer(pointer) };
        found.map(|s| (target, s)).ok_or_else(|| format!("$ref {reference} names nothing"))
    }

    /// Whether `instance` is valid against `schema` of `file`.
    fn valid(&self, file: &'static str, schema: &Value, instance: &Value) -> bool {
        let mut errors = Vec::new();
        self.check(file, schema, instance, "$", &mut errors);
        errors.is_empty()
    }

    /// Adds to `errors` every way `instance`, at `path`, breaks `schema` of
    /// `file`.
    fn check(&self, file: &'static str, schema: &Value, instance: &Value, path: &str, errors: &mut Vec<String>) {
        let s = match schema {
            Value::Bool(true) => return,
            Value::Bool(false) => {
                errors.push(format!("{path}: no value is allowed here"));
                return;
            }
            Value::Object(s) => s,
            _ => {
                errors.push(format!("{path}: the schema is not an object"));
                return;
            }
        };
        for (key, rule) in s {
            match key.as_str() {
                k if ANNOTATIONS.contains(&k) => {}
                // Applied by `properties` and `if`.
                "then" | "else" => {}
                "$defs" => {}
                "type" => {
                    let types: Vec<&str> = match rule {
                        Value::String(t) => vec![t.as_str()],
                        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                        _ => vec![],
                    };
                    if !types.iter().any(|t| is_type(instance, t)) {
                        errors.push(format!("{path}: {} is not of type {}", short(instance), types.join(" or ")));
                    }
                }
                "enum" => {
                    if !rule.as_array().is_some_and(|a| a.iter().any(|v| same(v, instance))) {
                        errors.push(format!("{path}: {} is not one of {rule}", short(instance)));
                    }
                }
                "const" => {
                    if !same(rule, instance) {
                        errors.push(format!("{path}: {} is not {rule}", short(instance)));
                    }
                }
                "required" => {
                    if let Value::Object(o) = instance {
                        for name in rule.as_array().into_iter().flatten().filter_map(Value::as_str) {
                            if !o.contains_key(name) {
                                errors.push(format!("{path}: `{name}` is missing"));
                            }
                        }
                    }
                }
                "properties" => {
                    if let (Value::Object(o), Value::Object(props)) = (instance, rule) {
                        for (name, sub) in props {
                            if let Some(v) = o.get(name) {
                                self.check(file, sub, v, &format!("{path}.{name}"), errors);
                            }
                        }
                    }
                }
                "additionalProperties" => {
                    if let Value::Object(o) = instance {
                        let named = s.get("properties").and_then(Value::as_object);
                        for (name, v) in o {
                            if !named.is_some_and(|p| p.contains_key(name)) {
                                match rule {
                                    Value::Bool(false) => errors.push(format!("{path}: `{name}` is not allowed")),
                                    sub => self.check(file, sub, v, &format!("{path}.{name}"), errors),
                                }
                            }
                        }
                    }
                }
                "prefixItems" => {
                    if let (Value::Array(a), Value::Array(subs)) = (instance, rule) {
                        for (i, (v, sub)) in a.iter().zip(subs).enumerate() {
                            self.check(file, sub, v, &format!("{path}[{i}]"), errors);
                        }
                    }
                }
                "items" => {
                    if let Value::Array(a) = instance {
                        let skip = s.get("prefixItems").and_then(Value::as_array).map_or(0, Vec::len);
                        for (i, v) in a.iter().enumerate().skip(skip) {
                            self.check(file, rule, v, &format!("{path}[{i}]"), errors);
                        }
                    }
                }
                "minItems" | "maxItems" => {
                    if let (Value::Array(a), Some(n)) = (instance, rule.as_u64()) {
                        let ok = if key == "minItems" { a.len() as u64 >= n } else { a.len() as u64 <= n };
                        if !ok {
                            errors.push(format!("{path}: {} items breaks {key} {n}", a.len()));
                        }
                    }
                }
                "uniqueItems" => {
                    if let (Value::Array(a), Value::Bool(true)) = (instance, rule) {
                        if (1..a.len()).any(|i| a[..i].iter().any(|b| same(b, &a[i]))) {
                            errors.push(format!("{path}: items repeat"));
                        }
                    }
                }
                "minimum" | "exclusiveMinimum" => {
                    if let (Some(v), Some(m)) = (instance.as_f64(), rule.as_f64()) {
                        let ok = if key == "minimum" { v >= m } else { v > m };
                        if instance.is_number() && !ok {
                            errors.push(format!("{path}: {v} breaks {key} {m}"));
                        }
                    }
                }
                "minLength" => {
                    if let (Value::String(t), Some(n)) = (instance, rule.as_u64()) {
                        if (t.chars().count() as u64) < n {
                            errors.push(format!("{path}: {t:?} is shorter than {n}"));
                        }
                    }
                }
                "pattern" => {
                    if let Value::String(t) = instance {
                        match rule.as_str() {
                            Some(KEBAB) => {
                                if !is_kebab(t) {
                                    errors.push(format!("{path}: {t:?} does not match {KEBAB}"));
                                }
                            }
                            p => errors.push(format!("{path}: the validator knows no pattern {p:?}")),
                        }
                    }
                }
                "$ref" => match rule.as_str().map(|r| self.resolve(file, r)) {
                    Some(Ok((f, sub))) => self.check(f, sub, instance, path, errors),
                    Some(Err(e)) => errors.push(format!("{path}: {e}")),
                    None => errors.push(format!("{path}: $ref is not a string")),
                },
                "allOf" => {
                    for sub in rule.as_array().into_iter().flatten() {
                        self.check(file, sub, instance, path, errors);
                    }
                }
                "anyOf" => {
                    if !rule.as_array().is_some_and(|a| a.iter().any(|sub| self.valid(file, sub, instance))) {
                        errors.push(format!("{path}: {} matches none of anyOf", short(instance)));
                    }
                }
                "oneOf" => {
                    let n = rule.as_array().map_or(0, |a| a.iter().filter(|sub| self.valid(file, sub, instance)).count());
                    if n != 1 {
                        errors.push(format!("{path}: {} matches {n} of oneOf", short(instance)));
                    }
                }
                "if" => {
                    let branch = if self.valid(file, rule, instance) { s.get("then") } else { s.get("else") };
                    if let Some(sub) = branch {
                        self.check(file, sub, instance, path, errors);
                    }
                }
                other => errors.push(format!("{path}: the validator knows no keyword `{other}`")),
            }
        }
    }
}

/// Whether `v` is of JSON Schema type `t`; an integer is a number with no
/// fraction.
fn is_type(v: &Value, t: &str) -> bool {
    match t {
        "null" => v.is_null(),
        "boolean" => v.is_boolean(),
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "number" => v.is_number(),
        "integer" => v.is_i64() || v.is_u64() || v.as_f64().is_some_and(|f| f.is_finite() && f.fract() == 0.0),
        _ => false,
    }
}

/// JSON equality, numbers compared as numbers.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
        _ => a == b,
    }
}

/// `^[a-z0-9]+(-[a-z0-9]+)*$`.
fn is_kebab(t: &str) -> bool {
    !t.is_empty() && t.split('-').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// A value shortened for a message.
fn short(v: &Value) -> String {
    let text = v.to_string();
    match text.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The validator accepts the schema's examples and rejects broken ones.
    #[test]
    fn examples_validate_and_breakage_is_caught() {
        let s = Schemas::get();
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/schemas/examples");
        for entry in std::fs::read_dir(dir).expect("the examples") {
            let path = entry.expect("an entry").path();
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let file = if name.starts_with("events.") {
                "events.schema.json"
            } else if name.starts_with("view.") {
                "view.schema.json"
            } else {
                continue;
            };
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("JSON");
            assert_eq!(s.validate(file, &v), Vec::<String>::new(), "{name}");
        }
        let bad = [
            json!({"type": "plot-cursor", "detail": {"source": "keyboard", "x": 1.0}}),
            json!({"type": "plot-cursor", "detail": {"source": "nose", "x": 1.0, "locked": true}}),
            json!({"type": "plot-brush", "detail": {"source": "pointer", "x": [1.0], "y": null, "count": 0, "final": true}}),
            json!({"type": "plot-hover", "detail": {"source": "pointer", "series": "y", "index": -1, "x": 1, "y": null}}),
            json!({"type": "plot-ready", "detail": {"type": "Line", "cols": 1, "rows": 1}}),
            json!({"type": "plot-view", "detail": {"source": "wheel", "cause": "zoom", "final": true, "view": {"x": {"domain": [0, 1], "span": 1}}}}),
            json!({"type": "plot-legend", "detail": {"source": "pointer", "series": "a", "visible": true, "hidden": ["b", "b"]}}),
            json!({"type": "plot-nothing", "detail": null}),
            json!({"type": "plot-data", "detail": {"rows": 1, "dropped": 0, "late": 0, "version": 1.5}}),
        ];
        for b in bad {
            assert!(!s.validate("events.schema.json", &b).is_empty(), "accepted {b}");
        }
    }
}
