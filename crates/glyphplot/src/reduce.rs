//! The input reducer: one input event folded into the view state, for every
//! chart (REQ-INPUT-010, REQ-INPUT-040, REQ-KEY-010, REQ-TOUCH-010, ADR-113,
//! ADR-116).
//!
//! [`reduce`] reads no clock and keeps nothing but the view and the
//! [`Gesture`] in flight: where a drag started, whether a wheel zoom or a
//! keyboard brush is open. A chart supplies what the reducer cannot know
//! through [`Scene`]: its plot, its x domain, its hit test and its data. The
//! same list of inputs on two fresh charts of the same size gives the same
//! views and events (contract rule 17).
//!
//! The defaults (REQ-INPUT-040, the plan's "Keys"):
//!
//! - Pointer: a drag pans, Shift+drag brushes, Ctrl or ⌘ with the wheel and
//!   a pinch zoom about the pointer, the plain wheel scrolls the page (no
//!   change), a click selects the datum and locks the cursor, a double click
//!   (a `tap` of count 2) resets. The options `drag` (`pan`, `brush`, `zoom`)
//!   and `wheel` (`scroll`, `zoom`) change the first and the fourth. A click
//!   on a legend entry toggles its series on release (`up`); the `tap` the
//!   shell sends after every still mouse or pen click toggles nothing more,
//!   so a click toggles once (REQ-EVENT-010, `plot-legend`).
//! - Touch (REQ-TOUCH-010): one finger scrubs the cursor, two pan (`pan`), a
//!   pinch zooms about its centre, a tap selects, a double tap resets, a long
//!   press (`press`) starts a brush the finger then drags. A tap on a legend
//!   entry toggles its series; a finger's `down` and `up` there do not, so a
//!   tap toggles once.
//! - Keys (REQ-KEY-010): ← → (↓ ↑, and h j k l) step the cursor datum by
//!   datum, panning past the edge; Home and End go to the first and last
//!   datum in view, End also setting `follow`; PageUp and PageDown move the
//!   cursor a tenth of the view; Shift+← → pan a tenth; + (=) and − zoom
//!   about the cursor; 0 resets; Enter locks the cursor and selects its
//!   datum; Shift+Enter starts, then commits, a brush from the cursor; Escape
//!   hides the readout, clears the brush and unlocks; 1–9 toggle the nth
//!   series; `f` follows. Space (pause), `?` (the keys), a second Escape
//!   (blur) and `q` (quit) are the shell's and the CLI's: they change
//!   nothing here, and an input that changes nothing returns no bits.
//!
//! Events (REQ-EVENT-020, REQ-EVENT-030): `plot-hover` only when the hovered
//! datum changes, `null` when the cursor leaves the plot; `plot-view` with
//! `final: false` during a drag, a wheel zoom, a pinch or a two-finger pan
//! and `final: true` once when it ends (pointer up, a `pinch` or `pan` with
//! `end`, a key, the end of a wheel zoom). A wheel zoom ends at the first
//! later input that is not a zoom wheel or comes 150 ms or more after it, or
//! at a wheel of no delta, which the shell sends when the wheel has been idle
//! 150 ms.

use std::collections::BTreeMap;

use crate::cells::Axis;
use crate::chart::{CellPos, Changes};
use crate::events::{Brush, Cause, CursorChange, Event, Hover, Legend, Select, Selected, Source, Value, ViewChange};
use crate::hit::Hit;
use crate::input::{Input, Key, Mods, PointerType};
use crate::layout::Rect;
use crate::view::{Cursor, Motion, MotionKind, Point, Selection, View};

/// What a plain mouse drag does (the `drag` option, REQ-INPUT-040).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DragMode {
    /// Pan (the default).
    #[default]
    Pan,
    /// Brush a range.
    Brush,
    /// Brush a range, then zoom to it.
    Zoom,
}

/// What the plain wheel does (the `wheel` option, REQ-INPUT-040).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WheelMode {
    /// Scroll the page: no change (the default).
    #[default]
    Scroll,
    /// Zoom about the pointer.
    Zoom,
}

/// The options that change the reducer's defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interaction {
    /// What a plain mouse drag does.
    pub drag: DragMode,
    /// What the plain wheel does.
    pub wheel: WheelMode,
    /// Whether resets and released two-finger pans animate as a `motion`
    /// (REQ-VIEW-050); the shell turns it off under reduced motion.
    pub motion: bool,
}

/// The seconds an eased reset lasts.
pub const EASE_SECONDS: f64 = 0.25;
/// The seconds a kinetic pan lasts.
pub const KINETIC_SECONDS: f64 = 0.5;
/// The seconds of wheel idleness that end a wheel zoom (REQ-EVENT-030).
pub const WHEEL_IDLE: f64 = 0.15;
/// How far, in cells, a pointer may move between down and up and still
/// click.
pub const CLICK_SLOP: f64 = 0.5;

/// A chart's plot as the reducer maps positions on it: its cells and its
/// axes through `cells.rs` (REQ-CANVAS-020).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plot {
    /// The plot's cells.
    pub rect: Rect,
    /// The x axis over the plot's columns.
    pub x: Axis,
    /// The y axis over the plot's rows.
    pub y: Axis,
}

impl Plot {
    /// Whether `at` is inside the plot.
    pub fn contains(&self, at: CellPos) -> bool {
        let r = self.rect;
        at.x >= r.x as f64 && at.x < (r.x + r.w) as f64 && at.y >= r.y as f64 && at.y < (r.y + r.h) as f64
    }

    /// The x value at the centre of the dot column under `at`, through the
    /// cell mapping; columns off the plot extrapolate.
    pub fn x_at(&self, at: CellPos) -> f64 {
        let k = ((at.x - self.rect.x as f64) * self.x.per() as f64).floor();
        // `as` saturates, and NaN goes to 0.
        let k = k as i64;
        self.x.dot_center(k)
    }

    /// The x span a cell covers.
    pub fn per_cell(&self) -> f64 {
        if self.rect.w == 0 {
            0.0
        } else {
            (self.x.hi() - self.x.lo()) / self.rect.w as f64
        }
    }
}

/// What a chart tells the reducer about itself (REQ-INPUT-010).
pub trait Scene {
    /// The plot of the frame the input lands on, or `None` before any frame.
    fn plot(&self) -> Option<Plot>;
    /// The x domain drawn at `view` (its fitted default when null).
    fn x_domain(&self, view: &View) -> [f64; 2];
    /// The fitted x domain, what a reset returns to.
    fn fitted_x(&self) -> [f64; 2];
    /// The datum the cursor snaps to at `at` (REQ-HIT-010).
    fn snap(&self, view: &View, at: CellPos) -> Option<Hit>;
    /// The datum `step` rows on from the cursor's in its series, skipping
    /// hidden series' data; `None` past the end.
    fn step(&self, view: &View, from: &Cursor, step: i64) -> Option<Hit>;
    /// The datum of series `series` (any shown series when `None`) whose x
    /// is nearest `x`.
    fn near(&self, view: &View, x: f64, series: Option<&str>) -> Option<Hit>;
    /// The first (or last) shown datum whose x is in `domain`.
    fn edge(&self, view: &View, domain: [f64; 2], last: bool) -> Option<Hit>;
    /// The series' names, in order.
    fn series(&self) -> Vec<String>;
    /// The datum's row, column by column, for events.
    fn datum(&self, hit: &Hit) -> BTreeMap<String, Value>;
    /// The shown data whose x lies in `range`.
    fn count(&self, view: &View, range: [f64; 2]) -> u64;
    /// The series whose legend entry is at `at`.
    fn legend_at(&self, at: CellPos) -> Option<usize>;
}

/// What a drag does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragKind {
    /// Pans the x domain.
    Pan,
    /// Brushes an x range.
    Brush,
    /// Brushes an x range, then zooms to it.
    Zoom,
    /// Moves the cursor (one finger).
    Scrub,
    /// Started on a legend entry: toggles its series on release.
    Legend(usize),
}

/// A drag in flight: from a `down` to its `up`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// What it does.
    pub kind: DragKind,
    /// Where it started.
    pub from: CellPos,
    /// The x domain when it started.
    pub domain: [f64; 2],
    /// The x value it started at, a brush's anchor.
    pub anchor: f64,
    /// Whether it has moved past the click slop.
    pub moved: bool,
    /// What it is made with.
    pub source: Source,
}

/// The gesture in flight between the inputs of one gesture: the only state
/// the reducer keeps outside the view. Reset by `leave`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gesture {
    /// A drag in flight.
    pub drag: Option<Drag>,
    /// The play time of the last zoom wheel of an open wheel zoom.
    pub wheel: Option<f64>,
    /// Whether a pinch or a two-finger pan is open.
    pub touch: bool,
    /// A keyboard brush's anchor, between Shift+Enter and Shift+Enter.
    pub key_brush: Option<f64>,
}

/// `domain` zoomed by `factor` (below 1 zooms in) about `about`, held to a
/// span between a millionth of a millionth of its magnitude and 10^300.
pub fn zoom(domain: [f64; 2], about: f64, factor: f64) -> [f64; 2] {
    let [lo, hi] = domain;
    let about = if about.is_finite() && about >= lo && about <= hi { about } else { lo + (hi - lo) / 2.0 };
    let factor = if factor.is_finite() && factor > 0.0 { factor } else { 1.0 };
    let span = (hi - lo) * factor;
    let least = (lo.abs().max(hi.abs()) * 1e-12).max(f64::MIN_POSITIVE * 1e10);
    if !(span >= least && span <= 1e300) {
        return domain;
    }
    let left = (about - lo) / (hi - lo);
    let lo2 = about - span * left;
    [lo2, lo2 + span]
}

/// `domain` moved by `by` data units.
pub fn shift(domain: [f64; 2], by: f64) -> [f64; 2] {
    let moved = [domain[0] + by, domain[1] + by];
    if moved[0].is_finite() && moved[1].is_finite() && moved[0] < moved[1] {
        moved
    } else {
        domain
    }
}

/// The source of a pointer.
fn pointer_source(p: PointerType) -> Source {
    match p {
        PointerType::Touch => Source::Touch,
        _ => Source::Pointer,
    }
}

/// The reducer's working state for one input.
struct Step<'a> {
    view: &'a mut View,
    g: &'a mut Gesture,
    scene: &'a dyn Scene,
    opts: Interaction,
    events: Vec<Event>,
    t: f64,
}

impl Step<'_> {
    fn view_event(&mut self, source: Source, cause: Cause, done: bool) {
        self.events.push(Event::View(Box::new(ViewChange { source, cause, done, view: self.view.clone() })));
    }

    /// The x domain drawn now.
    fn domain(&self) -> [f64; 2] {
        self.scene.x_domain(self.view)
    }

    /// Sets the x domain, ending any follow.
    fn set_domain(&mut self, d: [f64; 2]) {
        self.view.x.domain = Some(d);
        self.view.x.follow = false;
    }

    /// Ends an open wheel zoom with its final `plot-view`.
    fn close_wheel(&mut self) {
        if self.g.wheel.take().is_some() {
            self.view_event(Source::Wheel, Cause::Zoom, true);
        }
    }

    /// Ends a motion that is done by now; with `freeze`, also one in flight,
    /// at where it is, so a new gesture starts from what is drawn.
    fn settle_motion(&mut self, freeze: bool) {
        if let Some(m) = self.view.motion {
            if m.done(self.t) {
                self.view.motion = None;
            } else if freeze {
                self.view.motion = None;
                self.view.x.domain = Some(m.at(self.t));
            }
        }
    }

    /// Points the cursor at `hit` (or nowhere), firing `plot-hover` when the
    /// datum changed (REQ-EVENT-020).
    fn cursor_to(&mut self, hit: Option<Hit>, source: Source) {
        let before = self.view.cursor.as_ref().map(|c| (c.series.clone(), c.index));
        let locked = self.view.cursor.as_ref().is_some_and(|c| c.locked);
        self.view.cursor = hit.as_ref().map(|h| Cursor {
            x: Some(h.x),
            y: h.y.is_finite().then_some(h.y),
            series: Some(h.series.clone()),
            index: Some(h.index as u64),
            locked,
        });
        let after = self.view.cursor.as_ref().map(|c| (c.series.clone(), c.index));
        if before != after {
            let detail = hit.map(|h| {
                Box::new(Hover {
                    source,
                    series: h.series.clone(),
                    index: h.index as u64,
                    x: h.x,
                    y: h.y.is_finite().then_some(h.y),
                    datum: self.scene.datum(&h),
                    aggregate: h.aggregate,
                    cell: crate::events::Cell { col: h.col as u32, row: h.row as u32 },
                })
            });
            self.events.push(Event::Hover(detail));
        }
        if let Some(k) = self.g.key_brush {
            if let Some(x) = self.view.cursor.as_ref().and_then(|c| c.x) {
                let range = [k.min(x), k.max(x)];
                self.view.selection.get_or_insert_with(Selection::default).x = Some(range);
                let count = self.scene.count(self.view, range);
                self.events.push(Event::Brush(Brush { source, x: Some(range), y: None, count, done: false }));
            }
        }
    }

    /// Moves the cursor to `hit` from the keyboard, panning when its datum
    /// lies past the view's edge, and saying so to a locked cursor's
    /// listeners.
    fn key_cursor(&mut self, hit: Option<Hit>) {
        let Some(h) = hit else { return };
        let [lo, hi] = self.domain();
        if h.x < lo || h.x > hi {
            let by = if h.x < lo { h.x - lo } else { h.x - hi };
            let d = shift([lo, hi], by);
            self.set_domain(d);
            self.view_event(Source::Keyboard, Cause::Pan, true);
        }
        self.cursor_to(Some(h), Source::Keyboard);
        if let Some(c) = self.view.cursor.as_ref().filter(|c| c.locked) {
            let x = c.x;
            self.events.push(Event::Cursor(CursorChange { source: Source::Keyboard, x, locked: true }));
        }
    }

    /// Selects `hit`'s datum and locks the cursor on it.
    fn select(&mut self, hit: Hit, source: Source) {
        self.cursor_to(Some(hit.clone()), source);
        let point = Point { series: hit.series.clone(), index: hit.index as u64 };
        let sel = self.view.selection.get_or_insert_with(Selection::default);
        sel.points = vec![point];
        let points = sel.points.clone();
        let was_locked = self.view.cursor.as_ref().is_some_and(|c| c.locked);
        if let Some(c) = &mut self.view.cursor {
            c.locked = true;
        }
        self.events.push(Event::Select(Box::new(Select {
            source,
            series: hit.series.clone(),
            index: hit.index as u64,
            x: hit.x,
            y: hit.y.is_finite().then_some(hit.y),
            datum: self.scene.datum(&hit),
            selection: Selected { points },
        })));
        if !was_locked {
            self.events.push(Event::Cursor(CursorChange { source, x: Some(hit.x), locked: true }));
        }
    }

    /// Toggles series `index` (REQ-KEY-010: 1–9; a legend click).
    fn toggle(&mut self, index: usize, source: Source) {
        let names = self.scene.series();
        let Some(name) = names.get(index) else { return };
        let visible = if let Some(at) = self.view.hidden.iter().position(|h| h == name) {
            self.view.hidden.remove(at);
            true
        } else {
            self.view.hidden.push(name.clone());
            false
        };
        if !visible && self.view.cursor.as_ref().and_then(|c| c.series.as_deref()) == Some(name.as_str()) {
            self.view.cursor = None;
            self.events.push(Event::Hover(None));
        }
        self.events.push(Event::Legend(Legend { source, series: name.clone(), visible, hidden: self.view.hidden.clone() }));
    }

    /// Back to the fitted view, eased when motion is on.
    fn reset(&mut self, source: Source) {
        let from = self.domain();
        let to = self.scene.fitted_x();
        self.view.x.domain = None;
        self.view.x.follow = false;
        self.view.y.domain = None;
        self.view.axes.clear();
        self.view.motion = (self.opts.motion && from != to).then_some(Motion { kind: MotionKind::Ease, t0: self.t.max(0.0), from, to, duration: EASE_SECONDS });
        self.view_event(source, Cause::Reset, true);
    }

    /// Zooms by `factor` about `about`.
    fn zoom_by(&mut self, about: f64, factor: f64, source: Source, done: bool) {
        let d = self.domain();
        let z = zoom(d, about, factor);
        if z != d {
            self.set_domain(z);
        }
        self.view_event(source, Cause::Zoom, done);
    }

    fn brush_event(&mut self, source: Source, done: bool) {
        let x = self.view.selection.as_ref().and_then(|s| s.x);
        let count = x.map_or(0, |r| self.scene.count(self.view, r));
        self.events.push(Event::Brush(Brush { source, x, y: None, count, done }));
    }

    fn pointer_move(&mut self, at: CellPos, pointer: PointerType, buttons: u8) {
        let source = pointer_source(pointer);
        let Some(plot) = self.scene.plot() else { return };
        if let Some(mut d) = self.g.drag {
            if buttons == 0 && pointer != PointerType::Touch {
                // The button was released outside the chart: the drag ended.
                self.end_drag(at);
                return self.pointer_move(at, pointer, buttons);
            }
            d.moved |= (at.x - d.from.x).abs() > CLICK_SLOP || (at.y - d.from.y).abs() > CLICK_SLOP;
            self.g.drag = Some(d);
            match d.kind {
                DragKind::Pan if d.moved => {
                    let by = -(at.x - d.from.x) * plot.per_cell();
                    self.set_domain(shift(d.domain, by));
                    self.view_event(source, Cause::Pan, false);
                }
                DragKind::Brush | DragKind::Zoom if d.moved => {
                    let x = plot.x_at(at);
                    self.view.selection.get_or_insert_with(Selection::default).x = Some([d.anchor.min(x), d.anchor.max(x)]);
                    self.brush_event(source, false);
                }
                DragKind::Scrub => {
                    let hit = self.scene.snap(self.view, at);
                    if hit.is_some() {
                        self.cursor_to(hit, source);
                    }
                }
                _ => {}
            }
            return;
        }
        if self.view.cursor.as_ref().is_some_and(|c| c.locked) {
            return;
        }
        if plot.contains(at) {
            let hit = self.scene.snap(self.view, at);
            self.cursor_to(hit, source);
        } else if self.view.cursor.is_some() {
            self.cursor_to(None, source);
        }
    }

    fn pointer_down(&mut self, at: CellPos, pointer: PointerType, buttons: u8, mods: Mods) {
        let Some(plot) = self.scene.plot() else { return };
        if buttons & 1 == 0 && pointer != PointerType::Touch {
            return;
        }
        let source = pointer_source(pointer);
        let kind = if let Some(i) = self.scene.legend_at(at) {
            DragKind::Legend(i)
        } else if !plot.contains(at) {
            return;
        } else if pointer == PointerType::Touch {
            DragKind::Scrub
        } else if mods.shift() {
            DragKind::Brush
        } else {
            match self.opts.drag {
                DragMode::Pan => DragKind::Pan,
                DragMode::Brush => DragKind::Brush,
                DragMode::Zoom => DragKind::Zoom,
            }
        };
        self.settle_motion(true);
        let anchor = plot.x_at(at);
        self.g.drag = Some(Drag { kind, from: at, domain: self.domain(), anchor, moved: false, source });
        if kind == DragKind::Scrub {
            let hit = self.scene.snap(self.view, at);
            if hit.is_some() {
                self.cursor_to(hit, source);
            }
        }
    }

    fn end_drag(&mut self, at: CellPos) {
        let Some(d) = self.g.drag.take() else { return };
        let source = d.source;
        match d.kind {
            DragKind::Legend(i) => {
                // A mouse's or pen's click toggles here, and its `tap` after
                // does not; a finger's toggles on its `tap` alone.
                if source == Source::Pointer && self.scene.legend_at(at) == Some(i) {
                    self.toggle(i, source);
                }
            }
            DragKind::Pan if d.moved => self.view_event(source, Cause::Pan, true),
            DragKind::Brush if d.moved => self.brush_event(source, true),
            DragKind::Zoom if d.moved => {
                let range = self.view.selection.as_mut().and_then(|s| s.x.take());
                if let Some(r) = range.and_then(crate::view::clean_domain) {
                    self.set_domain(r);
                    self.view_event(source, Cause::BrushZoom, true);
                }
            }
            DragKind::Scrub => {}
            _ => {
                if source == Source::Pointer {
                    if let Some(h) = self.scene.snap(self.view, at) {
                        self.select(h, source);
                    }
                }
            }
        }
    }

    fn key(&mut self, key: Key, mods: Mods) {
        let src = Source::Keyboard;
        let cursor = self.view.cursor.clone();
        let step = |s: &mut Step, by: i64| {
            let hit = match &s.view.cursor {
                Some(c) if c.series.is_some() && c.index.is_some() => s.scene.step(s.view, c, by),
                _ => {
                    let d = s.domain();
                    s.scene.edge(s.view, d, by < 0)
                }
            };
            s.key_cursor(hit);
        };
        match key {
            Key::Left | Key::Right if mods.shift() => {
                self.settle_motion(true);
                let d = self.domain();
                let by = (d[1] - d[0]) / 10.0 * if key == Key::Left { -1.0 } else { 1.0 };
                self.set_domain(shift(d, by));
                self.view_event(src, Cause::Pan, true);
            }
            Key::Left | Key::Down | Key::H | Key::J => {
                self.settle_motion(true);
                step(self, -1);
            }
            Key::Right | Key::Up | Key::K | Key::L => {
                self.settle_motion(true);
                step(self, 1);
            }
            Key::Home | Key::End => {
                self.settle_motion(true);
                let d = self.domain();
                let hit = self.scene.edge(self.view, d, key == Key::End);
                self.key_cursor(hit);
                if key == Key::End && !self.view.x.follow {
                    self.view.x.follow = true;
                    self.view_event(src, Cause::Follow, true);
                }
            }
            Key::F => {
                if !self.view.x.follow {
                    self.view.x.follow = true;
                    self.view_event(src, Cause::Follow, true);
                }
            }
            Key::PageUp | Key::PageDown => {
                self.settle_motion(true);
                let d = self.domain();
                let from = cursor.as_ref().and_then(|c| c.x).unwrap_or(d[0] + (d[1] - d[0]) / 2.0);
                let x = from + (d[1] - d[0]) / 10.0 * if key == Key::PageUp { -1.0 } else { 1.0 };
                let series = cursor.as_ref().and_then(|c| c.series.clone());
                let hit = self.scene.near(self.view, x, series.as_deref());
                self.key_cursor(hit);
            }
            Key::Plus | Key::Equals | Key::Minus => {
                self.settle_motion(true);
                let about = cursor.as_ref().and_then(|c| c.x).unwrap_or(f64::NAN);
                self.zoom_by(about, if key == Key::Minus { 1.25 } else { 0.8 }, src, true);
            }
            Key::Zero => {
                self.settle_motion(true);
                self.reset(src);
            }
            Key::Enter if mods.shift() => match self.g.key_brush.take() {
                Some(_) => self.brush_event(src, true),
                None => {
                    let d = self.domain();
                    let a = cursor.as_ref().and_then(|c| c.x).unwrap_or(d[0] + (d[1] - d[0]) / 2.0);
                    self.g.key_brush = Some(a);
                    self.view.selection.get_or_insert_with(Selection::default).x = Some([a, a]);
                    self.brush_event(src, false);
                }
            },
            Key::Enter => {
                if let Some(h) = cursor.filter(|c| c.series.is_some() && c.index.is_some()).and_then(|c| self.scene.step(self.view, &c, 0)) {
                    self.select(h, src);
                }
            }
            Key::Escape => {
                let had_brush = self.view.selection.as_ref().is_some_and(|s| s.x.is_some() || s.y.is_some()) || self.g.key_brush.is_some();
                self.g.key_brush = None;
                if had_brush {
                    if let Some(s) = &mut self.view.selection {
                        s.x = None;
                        s.y = None;
                    }
                    self.events.push(Event::Brush(Brush { source: src, x: None, y: None, count: 0, done: true }));
                }
                if let Some(c) = self.view.cursor.take() {
                    if c.locked {
                        self.events.push(Event::Cursor(CursorChange { source: src, x: None, locked: false }));
                    }
                    self.events.push(Event::Hover(None));
                }
            }
            Key::Digit(n) => self.toggle(usize::from(n.max(1)) - 1, src),
            Key::Space | Key::Question | Key::Q | Key::Other => {}
        }
    }
}

/// Folds one input into `view` (REQ-INPUT-010): the view and the gesture
/// change, and the changes and events come back. `scene` answers what the
/// chart knows; `opts` are its interaction options.
pub fn reduce(view: &mut View, g: &mut Gesture, input: &Input, scene: &dyn Scene, opts: Interaction) -> Changes {
    let before = view.clone();
    let t = input.t();
    let mut s = Step { view, g, scene, opts, events: Vec::new(), t };
    // A wheel zoom ends at the first later input that is not a zoom wheel, or
    // that comes after the idle time.
    let zoom_wheel = matches!(input, Input::Wheel { dx, dy, mods, .. } if (*dx != 0.0 || *dy != 0.0) && (mods.ctrl_or_meta() || opts.wheel == WheelMode::Zoom));
    if let Some(last) = s.g.wheel {
        if !zoom_wheel || t - last >= WHEEL_IDLE {
            s.close_wheel();
        }
    }
    s.settle_motion(false);
    match *input {
        Input::Move { x, y, pointer, buttons, .. } => s.pointer_move(CellPos::new(x, y), pointer, buttons),
        Input::Down { x, y, pointer, buttons, mods, .. } => s.pointer_down(CellPos::new(x, y), pointer, buttons, mods),
        Input::Up { x, y, .. } => s.end_drag(CellPos::new(x, y)),
        Input::Leave { .. } => {
            if let Some(d) = s.g.drag.take() {
                if d.moved && d.kind == DragKind::Pan {
                    s.view_event(d.source, Cause::Pan, true);
                } else if d.moved && matches!(d.kind, DragKind::Brush | DragKind::Zoom) {
                    s.brush_event(d.source, true);
                }
            }
            s.g.touch = false;
            if s.view.cursor.as_ref().is_some_and(|c| !c.locked) {
                s.cursor_to(None, Source::Pointer);
            }
        }
        Input::Wheel { x, y, dy, .. } if zoom_wheel => {
            s.settle_motion(true);
            let about = s.scene.plot().map_or(f64::NAN, |p| p.x_at(CellPos::new(x, y)));
            s.g.wheel = Some(t);
            s.zoom_by(about, 1.1f64.powf(dy), Source::Wheel, false);
        }
        Input::Wheel { .. } => {}
        Input::Pinch { x, y, scale, end, .. } => {
            s.settle_motion(true);
            let about = s.scene.plot().map_or(f64::NAN, |p| p.x_at(CellPos::new(x, y)));
            let factor = if scale > 0.0 && scale.is_finite() { 1.0 / scale } else { 1.0 };
            s.g.touch = !end;
            s.zoom_by(about, factor, Source::Touch, end);
        }
        Input::Pan { dx, end, velocity, .. } => {
            s.settle_motion(true);
            let per = s.scene.plot().map_or(0.0, |p| p.per_cell());
            let d = shift(s.domain(), -dx * per);
            s.set_domain(d);
            s.g.touch = !end;
            if end && opts.motion && velocity.is_finite() && velocity != 0.0 {
                let to = shift(d, -velocity * per * KINETIC_SECONDS / 3.0);
                s.view.x.domain = Some(to);
                s.view.motion = Some(Motion { kind: MotionKind::Kinetic, t0: t.max(0.0), from: d, to, duration: KINETIC_SECONDS });
            }
            s.view_event(Source::Touch, Cause::Pan, end);
        }
        Input::Tap { x, y, count, pointer, .. } => {
            let at = CellPos::new(x, y);
            let source = pointer_source(pointer);
            if count >= 2 {
                s.settle_motion(true);
                if s.scene.plot().is_some_and(|p| p.contains(at)) {
                    s.reset(source);
                }
            } else if let Some(i) = s.scene.legend_at(at) {
                // A mouse's or pen's click toggled on its `up`; this tap
                // follows it, so only a finger's tap toggles here.
                if pointer == PointerType::Touch {
                    s.toggle(i, source);
                }
            } else if s.scene.plot().is_some_and(|p| p.contains(at)) {
                if let Some(h) = s.scene.snap(s.view, at) {
                    s.select(h, source);
                }
            }
        }
        Input::Press { x, y, .. } => {
            let at = CellPos::new(x, y);
            if let Some(plot) = s.scene.plot().filter(|p| p.contains(at)) {
                let anchor = plot.x_at(at);
                s.g.drag = Some(Drag { kind: DragKind::Brush, from: at, domain: s.domain(), anchor, moved: true, source: Source::Touch });
                s.view.selection.get_or_insert_with(Selection::default).x = Some([anchor, anchor]);
                s.brush_event(Source::Touch, false);
            }
        }
        Input::Key { key, mods, .. } => s.key(key, mods),
    }
    let events = std::mem::take(&mut s.events);
    let view = &*s.view;
    let base = view.x != before.x
        || view.y != before.y
        || view.axes != before.axes
        || view.hidden != before.hidden
        || view.motion != before.motion
        || view.selection.as_ref().map(|s| &s.points) != before.selection.as_ref().map(|s| &s.points);
    let overlay = base || view != &before;
    Changes { overlay, base, events }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_point_under_the_pointer() {
        let z = zoom([0.0, 100.0], 25.0, 0.5);
        assert_eq!(z, [12.5, 62.5]);
        assert_eq!(zoom([0.0, 100.0], f64::NAN, 2.0), [-50.0, 150.0], "no anchor: the middle");
        assert_eq!(zoom([0.0, 1e-300], 0.0, 1e-300), [0.0, 1e-300], "too small a span is refused");
        assert_eq!(shift([0.0, 1.0], f64::INFINITY), [0.0, 1.0]);
    }
}
