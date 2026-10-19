//! The events a chart's reducer produces, in the shapes of
//! `spec/schemas/events.schema.json` (REQ-EVENT-010 to 030).
//!
//! `Chart::input` returns them in [`crate::chart::Changes`]; the caller
//! dispatches each as a `plot-*` `CustomEvent` (the element) or prints it
//! (the CLI). [`to_json`] writes a list as `[{type, detail}, …]`, what the
//! wasm module's `events_json()` returns. The reader-action events carry a
//! [`Source`]; none is produced by setting the view, data or options
//! (REQ-VIEW-040).

use std::collections::BTreeMap;

use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

use crate::view::{Point, View};

/// What the reader acted with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A mouse or a pen.
    Pointer,
    /// A finger.
    Touch,
    /// A key.
    Keyboard,
    /// A wheel.
    Wheel,
}

/// A value of a datum's row, as the data table shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    /// A number, a timestamp in epoch ms or a duration in ms; NaN writes
    /// null.
    Number(f64),
    /// A category's or a string's label.
    Text(String),
    /// Missing.
    Null,
}

/// The column's summary of y when a dot column holds more than one datum
/// (REQ-HIT-020).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Aggregate {
    /// Data in the dot column.
    pub count: u64,
    /// The y of its first datum.
    pub first: f64,
    /// The y of its last datum.
    pub last: f64,
    /// Its least y.
    pub min: f64,
    /// Its greatest y.
    pub max: f64,
}

/// A cell of the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Cell {
    /// Column, from 0 at the left.
    pub col: u32,
    /// Row, from 0 at the top.
    pub row: u32,
}

/// `plot-hover`'s detail: the datum under the cursor (REQ-EVENT-020).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Hover {
    /// What the reader acted with.
    pub source: Source,
    /// The hovered series' name.
    pub series: String,
    /// The snapped datum's row.
    pub index: u64,
    /// The snapped datum's x.
    pub x: f64,
    /// The snapped datum's y, null when missing.
    pub y: Option<f64>,
    /// The snapped datum's row, column by column.
    pub datum: BTreeMap<String, Value>,
    /// The dot column's summary when it holds more than one datum
    /// (REQ-HIT-020).
    pub aggregate: Option<Aggregate>,
    /// The cell the cursor is in.
    pub cell: Cell,
}

/// The selection after a select.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Selected {
    /// Every selected datum.
    pub points: Vec<Point>,
}

/// `plot-select`'s detail.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Select {
    /// What the reader acted with.
    pub source: Source,
    /// The selected series' name.
    pub series: String,
    /// The selected datum's row.
    pub index: u64,
    /// The selected datum's x.
    pub x: f64,
    /// The selected datum's y, null when missing.
    pub y: Option<f64>,
    /// The selected datum's row.
    pub datum: BTreeMap<String, Value>,
    /// The selection after the action.
    pub selection: Selected,
}

/// `plot-brush`'s detail: x and y both null is a cleared brush.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Brush {
    /// What the reader acted with.
    pub source: Source,
    /// The brushed x range.
    pub x: Option<[f64; 2]>,
    /// The brushed y range.
    pub y: Option<[f64; 2]>,
    /// Data inside the brush.
    pub count: u64,
    /// False while it is dragged, true when committed or cleared.
    #[serde(rename = "final")]
    pub done: bool,
}

/// What changed the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cause {
    /// A pan.
    Pan,
    /// A zoom.
    Zoom,
    /// A reset to the fitted view.
    Reset,
    /// A zoom to a brushed range.
    BrushZoom,
    /// Following the live end.
    Follow,
}

/// `plot-view`'s detail (REQ-EVENT-030).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ViewChange {
    /// What the reader acted with.
    pub source: Source,
    /// What changed the view.
    pub cause: Cause,
    /// True on the gesture's last event.
    #[serde(rename = "final")]
    pub done: bool,
    /// The view after the change.
    pub view: View,
}

/// `plot-legend`'s detail.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Legend {
    /// What the reader acted with.
    pub source: Source,
    /// The toggled series.
    pub series: String,
    /// Whether it is now shown.
    pub visible: bool,
    /// Every hidden series after the toggle.
    pub hidden: Vec<String>,
}

/// `plot-cursor`'s detail.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CursorChange {
    /// What the reader acted with.
    pub source: Source,
    /// The cursor's x, null when it is gone.
    pub x: Option<f64>,
    /// Whether it is now locked.
    pub locked: bool,
}

/// `plot-ready`'s detail.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Ready {
    /// The chart type.
    #[serde(rename = "type")]
    pub kind: String,
    /// The grid's width.
    pub cols: u32,
    /// The grid's height.
    pub rows: u32,
}

/// `plot-error`'s detail.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Failure {
    /// The chart type asked for.
    #[serde(rename = "type")]
    pub kind: String,
    /// The error's message.
    pub message: String,
}

/// `plot-data`'s detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DataCommit {
    /// Rows held after the commit.
    pub rows: u64,
    /// Rows trimmed in this commit.
    pub dropped: u64,
    /// Appended rows dropped as late.
    pub late: u64,
    /// The data version.
    pub version: u64,
}

/// One event: its `plot-*` type and its detail (REQ-EVENT-010).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `plot-ready`.
    Ready(Ready),
    /// `plot-error`.
    Error(Failure),
    /// `plot-hover`; `None` when the cursor leaves the plot (REQ-EVENT-020).
    Hover(Option<Box<Hover>>),
    /// `plot-select`.
    Select(Box<Select>),
    /// `plot-brush`.
    Brush(Brush),
    /// `plot-view` (REQ-EVENT-030).
    View(Box<ViewChange>),
    /// `plot-legend`.
    Legend(Legend),
    /// `plot-cursor`.
    Cursor(CursorChange),
    /// `plot-data`.
    Data(DataCommit),
}

impl Event {
    /// The event's type, `plot-…`.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Ready(_) => "plot-ready",
            Event::Error(_) => "plot-error",
            Event::Hover(_) => "plot-hover",
            Event::Select(_) => "plot-select",
            Event::Brush(_) => "plot-brush",
            Event::View(_) => "plot-view",
            Event::Legend(_) => "plot-legend",
            Event::Cursor(_) => "plot-cursor",
            Event::Data(_) => "plot-data",
        }
    }

    /// The event as JSON, `{type, detail}`.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

impl Serialize for Event {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_struct("Event", 2)?;
        m.serialize_field("type", self.name())?;
        match self {
            Event::Ready(d) => m.serialize_field("detail", d)?,
            Event::Error(d) => m.serialize_field("detail", d)?,
            Event::Hover(d) => m.serialize_field("detail", d)?,
            Event::Select(d) => m.serialize_field("detail", d)?,
            Event::Brush(d) => m.serialize_field("detail", d)?,
            Event::View(d) => m.serialize_field("detail", d)?,
            Event::Legend(d) => m.serialize_field("detail", d)?,
            Event::Cursor(d) => m.serialize_field("detail", d)?,
            Event::Data(d) => m.serialize_field("detail", d)?,
        }
        m.end()
    }
}

/// Events as one JSON array, `[{type, detail}, …]`: what `events_json()`
/// returns.
pub fn to_json(events: &[Event]) -> String {
    serde_json::to_string(events).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn events_serialise_as_the_schema_examples() {
        let hover = Hover {
            source: Source::Touch,
            series: "y".into(),
            index: 700_123,
            x: 700_123.0,
            y: Some(0.42),
            datum: BTreeMap::from([("x".to_string(), Value::Number(700_123.0)), ("y".to_string(), Value::Null)]),
            aggregate: Some(Aggregate { count: 7042, first: 0.4, last: 0.43, min: -0.97, max: 0.99 }),
            cell: Cell { col: 30, row: 6 },
        };
        let v: serde_json::Value = serde_json::from_str(&Event::Hover(Some(Box::new(hover))).to_json()).unwrap();
        assert_eq!(v["type"], "plot-hover");
        assert_eq!(v["detail"]["aggregate"]["count"], 7042);
        assert_eq!(v["detail"]["datum"]["y"], serde_json::Value::Null);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&Event::Hover(None).to_json()).unwrap(), json!({"type": "plot-hover", "detail": null}));
        let brush = Event::Brush(Brush { source: Source::Keyboard, x: None, y: None, count: 0, done: true });
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{}/../../spec/schemas/examples/events.brush-cleared.json", env!("CARGO_MANIFEST_DIR"))).unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&brush.to_json()).unwrap(), expected);
        let cursor = Event::Cursor(CursorChange { source: Source::Keyboard, x: Some(1_760_043_200_000.0), locked: true });
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&to_json(&[cursor])).unwrap(),
            json!([{"type": "plot-cursor", "detail": {"source": "keyboard", "x": 1_760_043_200_000.0, "locked": true}}])
        );
        let view = Event::View(Box::new(ViewChange { source: Source::Wheel, cause: Cause::BrushZoom, done: false, view: View::default() }));
        let v: serde_json::Value = serde_json::from_str(&view.to_json()).unwrap();
        assert_eq!((v["detail"]["cause"].as_str(), v["detail"]["final"].as_bool()), (Some("brush-zoom"), Some(false)));
        assert_eq!(v["detail"]["view"]["version"], 1);
    }
}
