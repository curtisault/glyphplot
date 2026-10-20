//! The view state: everything interactive about a chart, in the shape of
//! `spec/schemas/view.schema.json` (REQ-VIEW-010 to 050, ADR-113).
//!
//! A frame is a function of the data, the options, this view and `t`. Every
//! domain is in data units (epoch ms for time) and `null` means the chart's
//! fitted default (REQ-VIEW-020). A view animation is a [`Motion`] record
//! stamped with play time: the x domain at any `t` is
//! [`View::x_domain_at`], so a frame is determined without replaying input
//! (REQ-VIEW-050). While a motion is in flight `x.domain` already holds where
//! it ends (null for a reset), so the record folds into it by being dropped.
//!
//! [`View`] is the whole state, as `Chart::view` returns it; [`ViewPatch`]
//! is a whole or partial view merged key by key, a key set to null returning
//! to its default (REQ-VIEW-030).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

/// How the y domain follows the data (view.schema.json `$defs/axis.fit`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Fit to the data in the x domain.
    #[default]
    Visible,
    /// Fit to all the data.
    All,
    /// Keep `domain`.
    Fixed,
}

/// The x axis's view.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XView {
    /// The visible x range `[low, high]`; `None` fits the data.
    pub domain: Option<[f64; 2]>,
    /// Live mode: the domain keeps its span and ends at the newest datum.
    #[serde(default)]
    pub follow: bool,
}

/// A y scale's view.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisView {
    /// The visible range; `None` with `fit`.
    pub domain: Option<[f64; 2]>,
    /// How it follows the data.
    #[serde(default)]
    pub fit: Fit,
}

/// The crosshair.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    /// Its x in data units.
    pub x: Option<f64>,
    /// Its y in data units: the snapped datum's.
    pub y: Option<f64>,
    /// The series it snapped to, by name.
    pub series: Option<String>,
    /// The row of the datum it snapped to.
    pub index: Option<u64>,
    /// Kept when the pointer leaves: a click or Enter locks it, Escape
    /// unlocks it.
    #[serde(default)]
    pub locked: bool,
}

/// A datum by series and row.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    /// The series' name.
    pub series: String,
    /// The datum's row in the data as held now.
    pub index: u64,
}

/// What the reader brushed and selected.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// A brushed x range.
    pub x: Option<[f64; 2]>,
    /// A brushed y range.
    pub y: Option<[f64; 2]>,
    /// Selected data.
    #[serde(default)]
    pub points: Vec<Point>,
}

/// What kind of view animation a [`Motion`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MotionKind {
    /// An eased move from `from` to `to` (a zoom, a reset).
    Ease,
    /// A pan decelerating from its release velocity.
    Kinetic,
}

/// A view animation of the x domain in flight, stamped with play time
/// (REQ-VIEW-050).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    /// Eased or kinetic.
    pub kind: MotionKind,
    /// Play time in seconds at which it starts.
    pub t0: f64,
    /// The x domain at `t0`.
    pub from: [f64; 2],
    /// The x domain at `t0 + duration`.
    pub to: [f64; 2],
    /// Seconds it lasts.
    pub duration: f64,
}

impl Motion {
    /// Whether it has ended by play time `t`.
    pub fn done(&self, t: f64) -> bool {
        self.duration.is_nan() || self.duration <= 0.0 || t >= self.t0 + self.duration
    }

    /// The x domain at play time `t`: `from` before `t0`, `to` from the end,
    /// and between them eased (a cubic in and out) or decelerating (a cubic
    /// out) — a function of `t` alone.
    pub fn at(&self, t: f64) -> [f64; 2] {
        if self.done(t) {
            return self.to;
        }
        if t.is_nan() || t <= self.t0 {
            return self.from;
        }
        let p = (t - self.t0) / self.duration;
        let e = match self.kind {
            MotionKind::Ease if p < 0.5 => 4.0 * p * p * p,
            MotionKind::Ease => 1.0 - (2.0 - 2.0 * p).powi(3) / 2.0,
            MotionKind::Kinetic => 1.0 - (1.0 - p).powi(3),
        };
        [self.from[0] + (self.to[0] - self.from[0]) * e, self.from[1] + (self.to[1] - self.from[1]) * e]
    }
}

/// The view state version this module writes.
pub const VERSION: u32 = 1;

fn version() -> u32 {
    VERSION
}

/// A chart's whole view state (view.schema.json, REQ-VIEW-010). Serialised
/// with every key present.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The shape's version, 1.
    #[serde(default = "version")]
    pub version: u32,
    /// The x axis's view.
    pub x: XView,
    /// The first y scale's view.
    pub y: AxisView,
    /// Further y scales, by the options' scale names.
    pub axes: BTreeMap<String, AxisView>,
    /// The crosshair, or none.
    pub cursor: Option<Cursor>,
    /// The selection, or none.
    pub selection: Option<Selection>,
    /// Series toggled off, by name; empty shows every series.
    pub hidden: Vec<String>,
    /// The page's time cursor in x units, or none.
    pub time: Option<f64>,
    /// A view animation in flight, or none (REQ-VIEW-050).
    pub motion: Option<Motion>,
}

impl Default for View {
    /// The fitted view: every domain null, no cursor, selection, hidden
    /// series, time cursor or motion.
    fn default() -> View {
        View {
            version: VERSION,
            x: XView::default(),
            y: AxisView::default(),
            axes: BTreeMap::new(),
            cursor: None,
            selection: None,
            hidden: Vec::new(),
            time: None,
            motion: None,
        }
    }
}

/// A domain as the view keeps it: low to high, finite, of some span; any
/// other is `None`, the fitted default.
pub fn clean_domain(d: [f64; 2]) -> Option<[f64; 2]> {
    let [a, b] = d;
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    (lo.is_finite() && hi.is_finite() && lo < hi && (hi - lo).is_finite()).then_some([lo, hi])
}

impl View {
    /// The view as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// A view from JSON: the fitted view with the given keys merged in.
    pub fn from_json(json: &str) -> Result<View, ViewError> {
        let mut v = View::default();
        v.apply(&ViewPatch::from_json(json)?);
        Ok(v)
    }

    /// The x domain to draw at play time `t`: a motion in flight's, else
    /// `x.domain`; `None` is the fitted default (REQ-VIEW-050).
    pub fn x_domain_at(&self, t: f64) -> Option<[f64; 2]> {
        match self.motion {
            Some(m) if !m.done(t) => Some(m.at(t)),
            _ => self.x.domain,
        }
    }

    /// Whether series `name` is hidden.
    pub fn is_hidden(&self, name: &str) -> bool {
        self.hidden.iter().any(|h| h == name)
    }

    /// Merges a patch, key by key; a key set to null returns to its default
    /// (REQ-VIEW-030). Domains are kept low to high; one of no span is the
    /// fitted default.
    pub fn apply(&mut self, p: &ViewPatch) {
        match &p.x {
            Patch::Keep => {}
            Patch::Reset => self.x = XView::default(),
            Patch::Set(x) => {
                x.domain.apply_to(&mut self.x.domain, |d| clean_domain(*d));
                if let Some(f) = x.follow {
                    self.x.follow = f;
                }
            }
        }
        apply_axis(&mut self.y, &p.y);
        match &p.axes {
            Patch::Keep => {}
            Patch::Reset => self.axes.clear(),
            Patch::Set(map) => {
                for (name, a) in map {
                    let axis = self.axes.entry(name.clone()).or_default();
                    apply_axis(axis, a);
                    if matches!(a, Patch::Reset) {
                        self.axes.remove(name);
                    }
                }
            }
        }
        match &p.cursor {
            Patch::Keep => {}
            Patch::Reset => self.cursor = None,
            Patch::Set(c) => {
                let cur = self.cursor.get_or_insert_with(Cursor::default);
                c.x.apply_to(&mut cur.x, |v| v.is_finite().then_some(*v));
                c.y.apply_to(&mut cur.y, |v| v.is_finite().then_some(*v));
                c.series.apply_to(&mut cur.series, |s| Some(s.clone()));
                c.index.apply_to(&mut cur.index, |i| Some(*i));
                if let Some(l) = c.locked {
                    cur.locked = l;
                }
            }
        }
        match &p.selection {
            Patch::Keep => {}
            Patch::Reset => self.selection = None,
            Patch::Set(s) => {
                let sel = self.selection.get_or_insert_with(Selection::default);
                s.x.apply_to(&mut sel.x, |d| clean_domain(*d));
                s.y.apply_to(&mut sel.y, |d| clean_domain(*d));
                if let Some(points) = &s.points {
                    sel.points = points.clone();
                }
            }
        }
        match &p.hidden {
            Patch::Keep => {}
            Patch::Reset => self.hidden.clear(),
            Patch::Set(h) => {
                self.hidden.clear();
                for name in h {
                    if !self.hidden.contains(name) {
                        self.hidden.push(name.clone());
                    }
                }
            }
        }
        p.time.apply_to(&mut self.time, |t| t.is_finite().then_some(*t));
        p.motion.apply_to(&mut self.motion, |m| {
            let ok = m.t0 >= 0.0 && m.duration > 0.0 && m.duration.is_finite() && clean_domain(m.from).is_some() && clean_domain(m.to).is_some();
            ok.then_some(*m)
        });
    }
}

fn apply_axis(axis: &mut AxisView, p: &Patch<AxisPatch>) {
    match p {
        Patch::Keep => {}
        Patch::Reset => *axis = AxisView::default(),
        Patch::Set(a) => {
            a.domain.apply_to(&mut axis.domain, |d| clean_domain(*d));
            if let Some(f) = a.fit {
                axis.fit = f;
            }
        }
    }
}

/// One key of a patch: left out (keep), null (back to the default) or a
/// value.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Patch<T> {
    /// The key is not given: keep what is there.
    #[default]
    Keep,
    /// The key is null: back to its default.
    Reset,
    /// The key's value.
    Set(T),
}

impl<T> Patch<T> {
    /// Applies the patch to an optional value: keep, clear, or set what
    /// `take` makes of the value (`None` clears).
    fn apply_to<U>(&self, slot: &mut Option<U>, take: impl Fn(&T) -> Option<U>) {
        match self {
            Patch::Keep => {}
            Patch::Reset => *slot = None,
            Patch::Set(v) => *slot = take(v),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Patch<T>, D::Error> {
        Ok(match Option::<T>::deserialize(d)? {
            None => Patch::Reset,
            Some(v) => Patch::Set(v),
        })
    }
}

/// A patch of the x view.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XPatch {
    /// The domain.
    #[serde(default)]
    pub domain: Patch<[f64; 2]>,
    /// Live mode.
    #[serde(default)]
    pub follow: Option<bool>,
}

/// A patch of a y scale's view.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisPatch {
    /// The domain.
    #[serde(default)]
    pub domain: Patch<[f64; 2]>,
    /// How it follows the data.
    #[serde(default)]
    pub fit: Option<Fit>,
}

/// A patch of the cursor.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CursorPatch {
    /// Its x.
    #[serde(default)]
    pub x: Patch<f64>,
    /// Its y.
    #[serde(default)]
    pub y: Patch<f64>,
    /// Its series.
    #[serde(default)]
    pub series: Patch<String>,
    /// Its row.
    #[serde(default)]
    pub index: Patch<u64>,
    /// Whether it is locked.
    #[serde(default)]
    pub locked: Option<bool>,
}

/// A patch of the selection.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionPatch {
    /// The brushed x range.
    #[serde(default)]
    pub x: Patch<[f64; 2]>,
    /// The brushed y range.
    #[serde(default)]
    pub y: Patch<[f64; 2]>,
    /// The selected data.
    #[serde(default)]
    pub points: Option<Vec<Point>>,
}

/// A whole or partial view, merged into a view key by key (REQ-VIEW-030).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewPatch {
    /// The shape's version; only 1 is read.
    #[serde(default)]
    pub version: Option<u32>,
    /// The x view.
    #[serde(default)]
    pub x: Patch<XPatch>,
    /// The first y scale's view.
    #[serde(default)]
    pub y: Patch<AxisPatch>,
    /// Further y scales; a scale set to null returns to its default.
    #[serde(default)]
    pub axes: Patch<BTreeMap<String, Patch<AxisPatch>>>,
    /// The cursor.
    #[serde(default)]
    pub cursor: Patch<CursorPatch>,
    /// The selection.
    #[serde(default)]
    pub selection: Patch<SelectionPatch>,
    /// Hidden series.
    #[serde(default)]
    pub hidden: Patch<Vec<String>>,
    /// The time cursor.
    #[serde(default)]
    pub time: Patch<f64>,
    /// A view animation.
    #[serde(default)]
    pub motion: Patch<Motion>,
}

/// A view or patch that does not fit view.schema.json.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewError(pub String);

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "view: {}", self.0)
    }
}

impl std::error::Error for ViewError {}

/// A value set, or null for none.
fn set_or_reset<T: Clone>(v: &Option<T>) -> Patch<T> {
    match v {
        Some(v) => Patch::Set(v.clone()),
        None => Patch::Reset,
    }
}

impl ViewPatch {
    /// A patch from JSON text; an empty text is the empty patch.
    pub fn from_json(json: &str) -> Result<ViewPatch, ViewError> {
        if json.trim().is_empty() {
            return Ok(ViewPatch::default());
        }
        let p: ViewPatch = serde_json::from_str(json).map_err(|e| ViewError(e.to_string()))?;
        match p.version {
            Some(v) if v != VERSION => Err(ViewError(format!("version {v}: this chart reads version {VERSION}"))),
            _ => Ok(p),
        }
    }

    /// The patch that sets every key to `view`'s, so applying it to any view
    /// gives `view` (REQ-VIEW-030, contract rule 16).
    pub fn whole(view: &View) -> ViewPatch {
        let axis = |a: &AxisView| Patch::Set(AxisPatch { domain: set_or_reset(&a.domain), fit: Some(a.fit) });
        ViewPatch {
            version: Some(VERSION),
            x: Patch::Set(XPatch { domain: set_or_reset(&view.x.domain), follow: Some(view.x.follow) }),
            y: axis(&view.y),
            axes: Patch::Set(view.axes.iter().map(|(k, a)| (k.clone(), axis(a))).collect()),
            cursor: match &view.cursor {
                None => Patch::Reset,
                Some(c) => Patch::Set(CursorPatch {
                    x: set_or_reset(&c.x),
                    y: set_or_reset(&c.y),
                    series: set_or_reset(&c.series),
                    index: set_or_reset(&c.index),
                    locked: Some(c.locked),
                }),
            },
            selection: match &view.selection {
                None => Patch::Reset,
                Some(s) => Patch::Set(SelectionPatch { x: set_or_reset(&s.x), y: set_or_reset(&s.y), points: Some(s.points.clone()) }),
            },
            hidden: Patch::Set(view.hidden.clone()),
            time: set_or_reset(&view.time),
            motion: set_or_reset(&view.motion),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../../spec/schemas/examples/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// REQ-VIEW-020: a fresh view is every domain null, and a key set to null
    /// returns to the fitted default.
    #[test]
    fn null_means_fitted() {
        let fresh = View::default();
        assert_eq!(fresh.x.domain, None);
        assert_eq!(fresh.y.domain, None);
        assert_eq!(fresh.x_domain_at(3.0), None);
        let mut v = View::from_json(&example("view.json")).unwrap();
        assert_eq!(v.x.domain, Some([1_760_000_000_000.0, 1_760_086_400_000.0]));
        v.apply(&ViewPatch::from_json(r#"{"x":null,"cursor":null,"hidden":null,"motion":null,"selection":null,"axes":null}"#).unwrap());
        assert_eq!(v.x, XView::default());
        assert_eq!((v.cursor.clone(), v.selection.clone(), v.hidden.clone(), v.motion), (None, None, vec![], None));
        v.apply(&ViewPatch::from_json(r#"{"x":{"domain":[5,1]}}"#).unwrap());
        assert_eq!(v.x.domain, Some([1.0, 5.0]), "kept low to high");
        v.apply(&ViewPatch::from_json(r#"{"x":{"domain":[2,2]}}"#).unwrap());
        assert_eq!(v.x.domain, None, "no span is the fitted default");
    }

    /// REQ-VIEW-030: a partial view merges key by key, and the whole view
    /// round-trips through JSON and through a patch.
    #[test]
    fn views_merge_and_round_trip() {
        let mut v = View::from_json(&example("view.json")).unwrap();
        let before = v.clone();
        v.apply(&ViewPatch::from_json(&example("view.partial.json")).unwrap());
        assert!(v.x.follow);
        assert_eq!(v.x.domain, before.x.domain, "follow alone keeps the domain");
        assert_eq!((v.cursor.clone(), v.hidden.len()), (None, 0));
        let back = View::from_json(&before.to_json()).unwrap();
        assert_eq!(back, before);
        let mut fresh = View::default();
        fresh.apply(&ViewPatch::whole(&before));
        assert_eq!(fresh, before);
        assert!(ViewPatch::from_json(r#"{"x":{"span":3}}"#).is_err());
        assert!(ViewPatch::from_json(r#"{"version":2}"#).is_err());
    }

    /// REQ-VIEW-050: the x domain at `t` is a function of the view and `t`
    /// alone: the motion's start before it, its end after it, eased between,
    /// and `x.domain` once it is done.
    #[test]
    fn motion_frames_depend_only_on_t() {
        let m = Motion { kind: MotionKind::Ease, t0: 1.0, from: [0.0, 100.0], to: [40.0, 60.0], duration: 0.5 };
        let v = View { x: XView { domain: Some([40.0, 60.0]), follow: false }, motion: Some(m), ..View::default() };
        assert_eq!(v.x_domain_at(0.0), Some([0.0, 100.0]));
        assert_eq!(v.x_domain_at(1.25), Some([20.0, 80.0]), "halfway, eased symmetrically");
        assert_eq!(v.x_domain_at(1.5), Some([40.0, 60.0]));
        assert_eq!(v.x_domain_at(9.0), Some([40.0, 60.0]));
        let order: Vec<_> = [6.0, 1.0, 1.1, 0.0, 1.1].iter().map(|&t| v.x_domain_at(t)).collect();
        assert_eq!(order[2], order[4], "the same t, the same domain, in any order");
        let k = Motion { kind: MotionKind::Kinetic, ..m };
        let a = k.at(1.1);
        assert!(a[0] > m.at(1.1)[0], "kinetic decelerates: ahead of the ease early on");
    }
}
