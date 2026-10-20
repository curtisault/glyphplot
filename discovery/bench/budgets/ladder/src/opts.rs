//! Step 4: options and meta as JSON through serde, as unicel's are.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    pub title: Option<String>,
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
    pub time_axis: bool,
    pub downsample: bool,
    pub colors: Vec<String>,
    pub legend: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OptionSpec {
    name: &'static str,
    kind: &'static str,
    default: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    slug: &'static str,
    name: &'static str,
    columns: Vec<&'static str>,
    interactions: Vec<&'static str>,
    options: Vec<OptionSpec>,
}

/// Every chart this module holds, as JSON.
#[wasm_bindgen]
pub fn metas_json() -> String {
    let mut v = vec![Meta {
        slug: "line",
        name: "Line",
        columns: vec!["x", "y"],
        interactions: vec!["pan", "zoom", "crosshair"],
        options: vec![
            OptionSpec { name: "timeAxis", kind: "bool", default: serde_json::Value::Bool(false) },
            OptionSpec { name: "downsample", kind: "bool", default: serde_json::Value::Bool(true) },
            OptionSpec { name: "yMin", kind: "number", default: serde_json::Value::Null },
        ],
    }];
    if cfg!(feature = "candle") {
        v.push(Meta { slug: "candles", name: "Candlestick", columns: vec!["t", "open", "high", "low", "close"], interactions: vec!["pan", "zoom", "crosshair"], options: vec![] });
    }
    serde_json::to_string(&v).unwrap_or_default()
}
