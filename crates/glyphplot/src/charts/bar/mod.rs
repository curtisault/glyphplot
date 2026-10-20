//! The `bar` family: values by category as bars, vertical or horizontal,
//! grouped or stacked (REQ-BAR-010, REQ-BAR-020). Its one type is `bar`, in
//! the mode `bar`; `histogram` (REQ-BAR-030) and `waterfall` join it later.
//!
//! Orientation and stacking are options, not modes: `orient` (`vertical`,
//! `horizontal`) and `stack` (`false` groups several series side by side,
//! `true` stacks them). They combine freely and draw the same data the same
//! way, turned or piled, as discovery/charts.md lays the family out ("`bar`,
//! mode `bar`"; "`bar`, options `group`, `stack`") and as the site's
//! catalogue plays every bar example in the mode `bar`; modes stay for what
//! draws the data differently, binning it (`histogram`) or running totals
//! (`waterfall`). Grouping is not an option of its own: several series not
//! stacked are grouped.
//!
//! The data is the `counts` shape (a category column and one or more value
//! columns) or the `values` shape (one value column, each row its own
//! category), one category a row in the data's order (`data.rs`). The
//! categories lie on a band scale, every one the same whole cells
//! (REQ-SCALE-040); the value axis is snapped to rows under 20 rows
//! (REQ-TICK-050); each bar's free end is at an eighth of a cell
//! (REQ-BAR-010); `geom.rs` says how. The view's x domain is in category
//! indices and picks the categories drawn, so zoom and pan move through
//! them; the value axis fits the categories in view unless the view fixes
//! it.
//!
//! The hit test is by band (REQ-BAR-020, REQ-HIT-010 for binned charts): the
//! cell under the pointer gives its category by division and, grouped, its
//! series by division within the band, in O(1) whatever the data's size; a
//! stack finds its segment among the category's shown series. Every plot
//! cell answers, the cells outside every band with the nearest category. The
//! readout names the category, the series and its value, written through
//! the series' column display (`format::Display`, REQ-DATA-100).
//!
//! In one ink the series fill with `█ ▓ ▒ ░` (REQ-MONO-030); a single series
//! with values below its baseline draws in the theme's up and down tokens,
//! `█` above and `▒` below (REQ-THEME-060).

mod data;
mod draw;
mod geom;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::annotate::{self, Annotation};
use crate::cells::Axis;
use crate::chart::{Accepted, Appended, CellPos, Changes, Charset, Chart, Env, Hit, MakeError, Meta, Mono, Part, Size, Table};
use crate::data::{Column, Data, DataError, Encode, Shape, Table as DataTable, Values};
use crate::events::Value;
use crate::format::{Locale, NumberFormat, Specifier};
use crate::grid::{Grid, NO_BACK};
use crate::input::Input;
use crate::layout::LegendPosition;
use crate::locale::LocalePack;
use crate::mono::Patterns;
use crate::options::Options;
use crate::palette::GROUND;
use crate::reduce::{reduce, DragMode, Gesture, Interaction, Plot, Scene, WheelMode};
use crate::registry::{Entry, Family};
use crate::table::pick_rows;
use crate::text;
use crate::view::{clean_domain, Cursor, Fit, View, ViewPatch};
use crate::Budget;

use data::Prepared;
use draw::BaseInput;
use geom::{Cats, Geom, GeomInput};

/// The `bar` type (REQ-BAR-010, REQ-CHART-020). Its minimum size is the
/// plan's `(2n + 6) × 6` for `n` bars (a cell a bar, a cell between, seven
/// cells of y gutter, two rows of x axis) stated for twelve bars, the
/// `categorical-12` fixture's: 30 × 6. A chart of more bars at that size
/// draws as many as there are cells, the rest a pan away.
pub static META: Meta = Meta {
    slug: "bar",
    name: "bar",
    note: "values by category as bars, vertical or horizontal, grouped or stacked",
    family: "bar",
    modes: &["bar"],
    shapes: &[Shape::Counts, Shape::Values],
    options: Some(
        r#"{"mode":"bar","orient":"vertical","stack":false,"padding":0.2,"baseline":0,"signed":"auto","title":"","encode":{},"legend":{"position":"auto"},"drag":"pan","wheel":"scroll","patterns":"auto","motion":false,"annotations":[]}"#,
    ),
    min: Size { cols: 30, rows: 6 },
    drop: &[Part::Title, Part::Legend, Part::Axes],
    mono: Mono::Authored,
    charset: Charset::Extended,
    budget: Budget::DEFAULT,
};

/// The family's types, by hand (REQ-REG-010, ADR-125).
pub const REGISTRY: &[Entry] = &[Entry { meta: &META, make }];

/// The family's registry entry.
pub const FAMILY: Family = Family { name: "bar", budget: Budget::DEFAULT, charts: REGISTRY };

/// A fresh `bar` chart for options already layered on [`META`]'s.
pub fn make(options: &Options) -> Result<Box<dyn Chart>, MakeError> {
    Ok(Box::new(BarChart::new(options)?))
}

/// The `signed` option: whether a series draws in up and down by sign
/// (REQ-THEME-060).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Signed {
    /// When the data holds one series and a value of it lies below the
    /// baseline (hiding series does not make the one left signed).
    Auto,
    /// Always.
    On,
    /// Never: the series' own colour.
    Off,
}

/// The options as the chart reads them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Config {
    /// Bars run left to right (`orient: "horizontal"`).
    horizontal: bool,
    /// Several series pile up instead of standing side by side.
    stack: bool,
    /// The share of each band left empty.
    padding: f64,
    /// The value bars start from.
    baseline: f64,
    /// Up and down by sign.
    signed: Signed,
    title: String,
    encode: Encode,
    legend: LegendPosition,
    interaction: Interaction,
    patterns: Patterns,
    /// The `annotations` option (REQ-ANNOT-010).
    annotations: Vec<Annotation>,
}

/// The options read: every key has a default in [`META`]'s (REQ-OPT-010).
fn config(o: &Options) -> Result<Config, MakeError> {
    o.mode(&META)?;
    let encode = o.object("encode").and_then(|m| serde_json::from_value::<Encode>(serde_json::Value::Object(m.clone())).ok()).unwrap_or_default();
    let legend = LegendPosition::from_option(o.object("legend").and_then(|m| m.get("position")).and_then(|p| p.as_str()).unwrap_or("auto"));
    let baseline = o.f64("baseline");
    let padding = o.f64("padding");
    Ok(Config {
        horizontal: o.choice("orient", &["vertical", "horizontal"]) == "horizontal",
        stack: o.bool("stack"),
        padding: if padding.is_finite() { geom::within(padding, 0.0, 1.0) } else { 0.2 },
        baseline: if baseline.is_finite() { baseline } else { 0.0 },
        signed: match o.choice("signed", &["auto", "on", "off"]) {
            "on" => Signed::On,
            "off" => Signed::Off,
            _ => Signed::Auto,
        },
        title: o.str("title").to_string(),
        encode,
        legend,
        interaction: Interaction {
            drag: match o.choice("drag", &["pan", "brush", "zoom"]) {
                "brush" => DragMode::Brush,
                "zoom" => DragMode::Zoom,
                _ => DragMode::Pan,
            },
            wheel: if o.choice("wheel", &["scroll", "zoom"]) == "zoom" { WheelMode::Zoom } else { WheelMode::Scroll },
            motion: o.bool("motion"),
        },
        patterns: Patterns::from_option(o.choice("patterns", &["auto", "on", "off"])),
        annotations: annotate::parse(o.get("annotations"))?,
    })
}

/// What the geometry depends on (REQ-LAYOUT-010).
#[derive(Clone, Debug, PartialEq)]
struct GeomKey {
    cols: usize,
    rows: usize,
    opts: u64,
    data: u64,
    /// The requested x domain at play time, as bits.
    x: Option<[u64; 2]>,
    y: Option<[u64; 2]>,
    fit: Fit,
    hidden: Vec<String>,
    locale: u64,
}

/// What the base depends on besides the geometry.
#[derive(Clone, Debug, PartialEq)]
struct BaseKey {
    geom: GeomKey,
    paper: bool,
    mono: bool,
}

/// The last frame's size and play time, which input and hit tests use.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FrameCtx {
    cols: usize,
    rows: usize,
    t: f64,
}

/// A value extent kept by what it was found for: data version, the
/// categories, the shown series and the stacking.
type ExtentKey = (u64, usize, usize, Vec<usize>, bool);

/// A `bar` chart (REQ-BAR-010): the [`Chart`] the registry makes for the
/// type `bar`.
pub struct BarChart {
    options: Options,
    cfg: Config,
    opts_version: u64,
    data: Prepared,
    data_version: u64,
    view: View,
    gesture: Gesture,
    extent: RefCell<Option<(ExtentKey, Option<[f64; 2]>)>>,
    geom: Option<(GeomKey, Geom)>,
    ctx: Option<FrameCtx>,
    pack: LocalePack,
    locale: Locale,
    locale_version: u64,
    base: Grid,
    base_key: Option<BaseKey>,
    base_draws: u64,
}

impl BarChart {
    /// A chart with no data for options layered on [`META`]'s; a mode not
    /// listed is an error naming the valid ones (REQ-CHART-030).
    pub fn new(options: &Options) -> Result<BarChart, MakeError> {
        let options = options.for_meta(&META);
        let cfg = config(&options)?;
        Ok(BarChart {
            options,
            cfg,
            opts_version: 0,
            data: Prepared::empty(),
            data_version: 0,
            view: View::default(),
            gesture: Gesture::default(),
            extent: RefCell::new(None),
            geom: None,
            ctx: None,
            pack: LocalePack::default(),
            locale: Locale::from_pack(&LocalePack::default()),
            locale_version: 0,
            base: Grid::new(1, 1),
            base_key: None,
            base_draws: 0,
        })
    }

    /// How many times the base has been drawn: once a change of data,
    /// options, view domains, size or env (REQ-OVERLAY-010).
    pub fn base_draws(&self) -> u64 {
        self.base_draws
    }

    /// The options as read, defaults filled in, recording each key read
    /// (REQ-OPT-010).
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// The series shown at `view`, in order.
    fn shown(&self, view: &View) -> Vec<usize> {
        (0..self.data.series.len()).filter(|&s| !view.is_hidden(&self.data.series[s].name)).collect()
    }

    fn geom_key(&self, view: &View, t: f64, cols: usize, rows: usize) -> GeomKey {
        let bits = |d: [f64; 2]| [d[0].to_bits(), d[1].to_bits()];
        GeomKey {
            cols,
            rows,
            opts: self.opts_version,
            data: self.data_version,
            x: view.x_domain_at(t).map(bits),
            y: view.y.domain.and_then(clean_domain).map(bits),
            fit: view.y.fit,
            hidden: view.hidden.clone(),
            locale: self.locale_version,
        }
    }

    /// The value extent of the shown series over categories `range`, kept
    /// by what it depends on.
    fn extent(&self, shown: &[usize], range: std::ops::Range<usize>) -> Option<[f64; 2]> {
        let key = (self.data_version, range.start, range.end, shown.to_vec(), self.cfg.stack);
        let mut cache = self.extent.borrow_mut();
        if let Some((k, v)) = cache.as_ref() {
            if *k == key {
                return *v;
            }
        }
        let v = geom::extent(&self.data, shown, range, self.cfg.stack, self.cfg.baseline);
        *cache = Some((key, v));
        v
    }

    /// The geometry for `key` at `view`.
    fn make_geom(&self, key: &GeomKey, view: &View) -> Geom {
        let x = key.x.map(|b| [f64::from_bits(b[0]), f64::from_bits(b[1])]);
        let y = key.y.map(|b| [f64::from_bits(b[0]), f64::from_bits(b[1])]);
        let shown = self.shown(view);
        let all = key.fit == Fit::All;
        let n = self.data.len();
        let ext = |range: std::ops::Range<usize>| self.extent(&shown, if all { 0..n } else { range });
        let input = GeomInput { cols: key.cols, rows: key.rows, min: META.min, cfg: &self.cfg, x, y, shown: shown.clone(), locale: &self.locale };
        geom::make(&input, &self.data, &ext)
    }

    /// Makes sure the kept geometry is the current view's at play time `t`,
    /// at the last frame's size.
    fn ensure_geom(&mut self, t: f64) {
        let Some(ctx) = self.ctx else {
            self.geom = None;
            return;
        };
        let key = self.geom_key(&self.view, t, ctx.cols, ctx.rows);
        if self.geom.as_ref().map(|g| &g.0) != Some(&key) {
            let g = self.make_geom(&key, &self.view);
            self.geom = Some((key, g));
        }
    }

    /// The geometry of the current view at the last frame's size and time:
    /// the kept one when it is current, else made afresh; None before any
    /// frame.
    fn geom_now(&self) -> Option<Cow<'_, Geom>> {
        let ctx = self.ctx?;
        let key = self.geom_key(&self.view, ctx.t, ctx.cols, ctx.rows);
        match &self.geom {
            Some((k, g)) if *k == key => Some(Cow::Borrowed(g)),
            _ => Some(Cow::Owned(self.make_geom(&key, &self.view))),
        }
    }

    /// The x domain drawn at `view`, in category indices: the categories'
    /// drawn domain at the last frame's size (REQ-VIEW-020), or before any
    /// frame every category's `[−½, n − ½]` (the requested one when set).
    fn drawn_domain(&self, view: &View, geom: Option<&Geom>) -> Option<[f64; 2]> {
        let n = self.data.len();
        if n == 0 {
            return None;
        }
        let groups = if !self.cfg.stack { self.shown(view).len() } else { 1 };
        let cells = geom.and_then(|g| g.cats).map(|c| c.band.cells);
        match cells.and_then(|cells| Cats::new(n, view.x.domain, cells, groups, self.cfg.padding)) {
            Some(c) => Some(c.domain()),
            None => Some(view.x.domain.unwrap_or([-0.5, n as f64 - 0.5])),
        }
    }

    /// The hit of series `s`'s value in category `i`, placed on the plot
    /// where its bar's free end is drawn; at `at` when the category is not
    /// drawn.
    fn hit_of(&self, geom: Option<&Geom>, s: usize, i: usize, at: Option<CellPos>) -> Hit {
        let (mut col, mut row) = at.map_or((0, 0), |p| (p.x.max(0.0) as usize, p.y.max(0.0) as usize));
        if let Some(g) = geom {
            if at.is_none() {
                (col, row) = (g.plot.x, g.plot.y);
            }
            let placed = g.cats.and_then(|c| {
                let k = i.checked_sub(c.first).filter(|&k| k < c.count())?;
                let j = g.shown.iter().position(|&x| x == s)?;
                let seg = g.seg(k, j)?;
                let cross = if c.groups > 1 { (seg.cross.start + seg.cross.end - 1) / 2 } else { c.middle(k) };
                let along = seg.end_cell().clamp(0, g.value.cells.saturating_sub(1) as i64);
                g.cell(cross, along)
            });
            if let Some(p) = placed {
                (col, row) = p;
            }
        }
        Hit { series: self.data.series[s].name.clone(), series_index: s, index: i, x: i as f64, y: self.data.stored(s, i), col, row, aggregate: None }
    }

    /// The bar under `at` (REQ-BAR-020, REQ-HIT-010): the band under the
    /// pointer by division, its series by division within the band when
    /// grouped, else the stack's segment under the pointer, or the nearest.
    /// Every plot cell answers.
    fn hit_in(&self, geom: &Geom, at: CellPos) -> Option<Hit> {
        let p = geom.plot;
        let inside = at.x >= p.x as f64 && at.x < (p.x + p.w) as f64 && at.y >= p.y as f64 && at.y < (p.y + p.h) as f64;
        if !inside || geom.shown.is_empty() {
            return None;
        }
        let cats = geom.cats?;
        let (c, r) = (at.x.floor() as usize - p.x, at.y.floor() as usize - p.y);
        let (cross, along) = if geom.horizontal { (r, c as i64) } else { (c, (p.h - 1 - r) as i64) };
        let k = cats.at(cross);
        let j = if cats.groups > 1 {
            cats.bar_at(k, cross)
        } else if self.cfg.stack {
            let m = geom.shown.len();
            let mut best: Option<(i64, usize)> = None;
            for j in 0..m {
                let Some(cells) = geom.seg(k, j).map(|s| s.cells()) else { continue };
                if cells.is_empty() {
                    continue;
                }
                let d = if along < cells.start { cells.start - along } else { (along - (cells.end - 1)).max(0) };
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, j));
                }
            }
            best.map_or(0, |(_, j)| j)
        } else {
            0
        };
        Some(self.hit_of(Some(geom), geom.shown[j], cats.first + k, Some(at)))
    }

    /// The readout of the view's cursor: the category, then the series and
    /// its value as its column displays it (REQ-DATA-100), the category cut
    /// with `…` so the value fits the plot's width.
    fn readout(&self, geom: &Geom) -> Option<String> {
        let c = self.view.cursor.as_ref()?;
        let s = self.data.find(c.series.as_deref()?)?;
        let i = usize::try_from(c.index?).ok().filter(|&i| i < self.data.len())?;
        let mut tail = String::from("  ");
        tail.push_str(&self.data.series[s].name);
        tail.push(' ');
        tail.push_str(&self.value(s, i));
        let room = geom.plot.w.saturating_sub(text::width(&tail));
        let label = self.data.label(i, &self.locale);
        if room >= 2 && !label.is_empty() {
            Some(text::truncate(&label, room) + &tail)
        } else {
            Some(tail.trim_start().to_string())
        }
    }

    /// Series `s`'s value in category `i` as its column displays it, with
    /// its unit (REQ-DATA-100); `no value` when missing.
    fn value(&self, s: usize, i: usize) -> String {
        let v = self.data.stored(s, i);
        if !v.is_finite() {
            return "no value".to_string();
        }
        let d = &self.data.series[s].display;
        d.with_unit(&d.readout(v, &Specifier::default(), &self.locale))
    }

    /// The cursor's value text: its category, then each shown series' value
    /// there, up to four, then `and N more` (REQ-A11Y-030).
    fn value_text(&self) -> Option<String> {
        let c = self.view.cursor.as_ref()?;
        let i = usize::try_from(c.index?).ok().filter(|&i| i < self.data.len())?;
        let shown = self.shown(&self.view);
        let mut parts = vec![self.data.label(i, &self.locale)];
        for &s in shown.iter().take(4) {
            parts.push(format!("{} {}", self.data.series[s].name, self.value(s, i)));
        }
        let mut text = parts.join(", ");
        if shown.len() > 4 {
            text.push_str(&format!(", and {} more", shown.len() - 4));
        }
        Some(text)
    }

    /// The chart's label: what it draws, its series, its categories and its
    /// value range (REQ-A11Y-030).
    fn summary(&self, geom: Option<&Geom>) -> String {
        let what = match (self.cfg.horizontal, self.cfg.stack) {
            (false, false) => "bar chart",
            (true, false) => "horizontal bar chart",
            (false, true) => "stacked bar chart",
            (true, true) => "stacked horizontal bar chart",
        };
        let title = if self.cfg.title.is_empty() { String::new() } else { format!("{}: ", self.cfg.title) };
        let n = self.data.len();
        if n == 0 || self.data.series.is_empty() {
            return format!("{title}{what}, no data");
        }
        let names: Vec<&str> = self.data.series.iter().map(|s| s.name.as_str()).collect();
        let mut text = format!("{title}{what} of {} series ({}); {} categories", names.len(), names.join(", "), count(n, &self.locale));
        if let Some(c) = geom.and_then(|g| g.cats) {
            if c.count() < n {
                text.push_str(&format!(
                    ", {} in view from {} to {}",
                    count(c.count(), &self.locale),
                    self.data.label(c.first, &self.locale),
                    self.data.label(c.first + c.count() - 1, &self.locale)
                ));
            }
        }
        if let Some([a, b]) = self.extent(&self.shown(&self.view), 0..n) {
            let f = NumberFormat::new(&Specifier::default(), &self.locale);
            text.push_str(&format!("; values from {} to {}", f.format(a), f.format(b)));
        }
        text
    }

    /// Re-derives the cursor from its series and row after the data changed.
    fn resnap(&mut self) {
        let Some(c) = self.view.cursor.clone() else { return };
        let hit = c
            .series
            .as_deref()
            .and_then(|name| self.data.find(name))
            .zip(c.index.and_then(|i| usize::try_from(i).ok()))
            .filter(|&(_, i)| i < self.data.len())
            .map(|(s, i)| self.hit_of(None, s, i, None));
        self.view.cursor =
            hit.map(|h| Cursor { x: Some(h.x), y: h.y.is_finite().then_some(h.y), series: Some(h.series), index: Some(h.index as u64), locked: c.locked });
        let n = self.data.len() as u64;
        if let Some(sel) = &mut self.view.selection {
            sel.points.retain(|p| p.index < n);
        }
    }

    fn take_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        data.shape(META.shapes, &self.cfg.encode)?;
        let table = data.into_tables().into_iter().next().map(|(_, t)| t);
        self.data = table.map_or_else(Prepared::empty, |t| Prepared::new(t, &self.cfg.encode));
        self.geom = None;
        *self.extent.get_mut() = None;
        self.base_key = None;
        self.data_version += 1;
        self.resnap();
        Ok(Accepted { rows: self.data.rows(), version: self.data_version, warnings: self.data.warnings.as_slice().to_vec() })
    }
}

/// A count as captions write it: whole, grouped by the locale.
fn count(n: usize, locale: &Locale) -> String {
    match Specifier::parse("count", ",d") {
        Ok(spec) => NumberFormat::new(&spec, locale).format(n as f64),
        Err(_) => n.to_string(),
    }
}

/// `table`'s rows with `batch`'s after them: the same columns by name and
/// type, numbers held without an origin, category labels merged (new labels
/// after the old ones).
fn appended(table: &DataTable, batch: &DataTable) -> Result<DataTable, DataError> {
    let cols = table.columns();
    if batch.columns().len() != cols.len() {
        return Err(DataError::Append(format!("rows have {} columns; the data has {}", batch.columns().len(), cols.len())));
    }
    let mut out = Vec::with_capacity(cols.len());
    for c in cols {
        let Some(b) = batch.by_name(&c.name).filter(|b| b.kind == c.kind && b.origin == 0 && c.origin == 0) else {
            return Err(DataError::Append(format!("column `{}` is missing, of another type or held from an origin", c.name)));
        };
        let values = match (&c.values, &b.values) {
            (Values::Num(a), Values::Num(v)) => Values::Num(a.iter().chain(v).copied().collect()),
            (Values::Text(a), Values::Text(v)) => Values::Text(a.iter().chain(v).cloned().collect()),
            (Values::Codes(a), Values::Codes(v)) => {
                let mut labels = c.categories.clone();
                let mut codes = a.clone();
                for (r, &code) in v.iter().enumerate() {
                    let Some(l) = b.categories.get(code as usize).filter(|_| !b.is_missing(r)) else {
                        codes.push(u32::MAX);
                        continue;
                    };
                    let at = labels.iter().position(|x| x == l).unwrap_or_else(|| {
                        labels.push(l.clone());
                        labels.len() - 1
                    });
                    codes.push(at as u32);
                }
                let mut n = Column::categories(&c.name, labels, codes);
                n.role = c.role;
                out.push(n);
                continue;
            }
            _ => return Err(DataError::Append(format!("column `{}` holds values of another kind", c.name))),
        };
        let mut n = c.clone();
        n.values = values;
        out.push(n);
    }
    DataTable::from_columns(out)
}

impl Chart for BarChart {
    fn set_data(&mut self, data: Data) -> Result<Accepted, DataError> {
        self.take_data(data)
    }

    fn append(&mut self, batch: Data) -> Result<Appended, DataError> {
        let Some(old) = self.data.table.as_ref() else {
            let a = self.take_data(batch)?;
            return Ok(Appended { rows: a.rows, dropped: 0, late: 0, version: a.version });
        };
        let Some(new) = batch.first_table() else { return Err(DataError::Shape { expected: META.shapes.to_vec() }) };
        let table = appended(old, new)?;
        let a = self.take_data(Data::from_table(table))?;
        Ok(Appended { rows: a.rows, dropped: 0, late: 0, version: a.version })
    }

    fn set_options(&mut self, options: &Options) -> Result<(), MakeError> {
        let layered = options.for_meta(&META);
        let cfg = config(&layered)?;
        let encode_changed = cfg.encode != self.cfg.encode;
        self.options = layered;
        self.cfg = cfg;
        self.opts_version += 1;
        *self.extent.get_mut() = None;
        if encode_changed {
            let table = std::mem::replace(&mut self.data, Prepared::empty()).into_table();
            self.data = table.map_or_else(Prepared::empty, |t| Prepared::new(t, &self.cfg.encode));
            self.data_version += 1;
            self.resnap();
        }
        Ok(())
    }

    fn view(&self) -> &View {
        &self.view
    }

    fn set_view(&mut self, patch: &ViewPatch) {
        self.view.apply(patch);
    }

    fn input(&mut self, event: &Input) -> Changes {
        self.ensure_geom(event.t());
        let mut view = std::mem::take(&mut self.view);
        let mut gesture = std::mem::take(&mut self.gesture);
        let changes = reduce(&mut view, &mut gesture, event, &*self, self.cfg.interaction);
        self.view = view;
        self.gesture = gesture;
        changes
    }

    fn frame(&mut self, t: f64, env: &Env, grid: &mut Grid) {
        if *env.locale != self.pack {
            self.pack = env.locale.clone();
            self.locale = Locale::from_pack(env.locale);
            self.locale_version += 1;
        }
        let (cols, rows) = (grid.cols(), grid.rows());
        self.ctx = Some(FrameCtx { cols, rows, t });
        self.ensure_geom(t);
        let Some((key, geom)) = &self.geom else { return };
        let ground = if env.paper { GROUND } else { NO_BACK };
        let bkey = BaseKey { geom: key.clone(), paper: env.paper, mono: env.mono };
        if self.base_key.as_ref() != Some(&bkey) {
            if self.base.cols() != cols || self.base.rows() != rows {
                self.base.resize(cols, rows);
            }
            let input = BaseInput { cfg: &self.cfg, geom, view: &self.view, mono: env.mono, ground };
            draw::base(&mut self.base, &self.data, &input);
            self.base_key = Some(bkey);
            self.base_draws += 1;
        }
        grid.copy_from(&self.base);
        let text = self.readout(geom);
        draw::overlay(grid, geom, &self.data, &self.view, text.as_deref(), ground);
    }

    fn hit(&self, at: CellPos) -> Option<Hit> {
        let g = self.geom_now()?;
        self.hit_in(&g, at)
    }

    fn table(&self, max_rows: usize) -> Table {
        let geom = self.geom_now();
        let geom = geom.as_deref();
        let n = self.data.len();
        let mut columns = vec![self.data.cat_name.clone()];
        columns.extend(self.data.series.iter().map(|s| s.name.clone()));
        let (first, total) = match geom.and_then(|g| g.cats) {
            Some(c) => (c.first, c.count()),
            None if geom.is_some() => (0, 0),
            None => (0, n),
        };
        let lead = self.shown(&self.view).first().copied();
        let picked = pick_rows(total, max_rows, |k| lead.map_or(f64::NAN, |s| self.data.stored(s, first + k)));
        let rows: Vec<Vec<String>> = picked
            .into_iter()
            .map(|k| {
                let i = first + k;
                let mut cells = vec![self.data.label(i, &self.locale)];
                for s in 0..self.data.series.len() {
                    let v = self.data.stored(s, i);
                    let d = &self.data.series[s].display;
                    cells.push(if v.is_finite() { d.readout(v, &Specifier::default(), &self.locale) } else { String::new() });
                }
                cells
            })
            .collect();
        let name = if self.cfg.title.is_empty() { "bar chart" } else { self.cfg.title.as_str() };
        let caption = if rows.len() < total { format!("{name}, each bucket's least and greatest kept") } else { name.to_string() };
        Table {
            caption,
            columns,
            rows,
            total: total as u64,
            summary: self.summary(geom),
            value: self.value_text(),
            domain: self.drawn_domain(&self.view, geom).filter(|[a, b]| a < b),
        }
    }
}

impl Scene for BarChart {
    fn plot(&self) -> Option<Plot> {
        let (_, g) = self.geom.as_ref()?;
        let p = g.plot;
        let x = g.cat_axis().map_or(Axis::x(0.0, 1.0, p.w, 2), |a| Axis::x(a.lo(), a.hi(), p.w, 2));
        let [lo, hi] = g.value.domain;
        let y = if g.horizontal { Axis::y(lo, hi, p.h, 4) } else { Axis::y(lo, hi, g.value.cells, 4) };
        Some(Plot { rect: p, x, y })
    }

    fn x_domain(&self, view: &View) -> [f64; 2] {
        self.drawn_domain(view, self.geom.as_ref().map(|g| &g.1)).unwrap_or([-0.5, 0.5])
    }

    fn fitted_x(&self) -> [f64; 2] {
        let fitted = View { x: Default::default(), ..self.view.clone() };
        self.x_domain(&fitted)
    }

    fn snap(&self, _view: &View, at: CellPos) -> Option<Hit> {
        self.geom.as_ref().and_then(|(_, g)| self.hit_in(g, at))
    }

    fn step(&self, view: &View, from: &Cursor, step: i64) -> Option<Hit> {
        let s = self.data.find(from.series.as_deref()?)?;
        if view.is_hidden(&self.data.series[s].name) {
            return None;
        }
        let i = usize::try_from(i64::try_from(from.index?).ok()? + step).ok().filter(|&i| i < self.data.len())?;
        Some(self.hit_of(self.geom.as_ref().map(|g| &g.1), s, i, None))
    }

    fn near(&self, view: &View, x: f64, series: Option<&str>) -> Option<Hit> {
        let n = self.data.len();
        if n == 0 || !x.is_finite() {
            return None;
        }
        let shown = self.shown(view);
        let s = series.and_then(|name| self.data.find(name)).filter(|s| shown.contains(s)).or(shown.first().copied())?;
        let i = geom::within(x.round(), 0.0, (n - 1) as f64) as usize;
        Some(self.hit_of(self.geom.as_ref().map(|g| &g.1), s, i, None))
    }

    fn edge(&self, view: &View, domain: [f64; 2], last: bool) -> Option<Hit> {
        let n = self.data.len();
        let s = self.shown(view).first().copied()?;
        let (a, b) = (domain[0].min(domain[1]).ceil().max(0.0), domain[0].max(domain[1]).floor().min(n as f64 - 1.0));
        let ordered = a <= b;
        if !ordered {
            return None;
        }
        let i = if last { b } else { a } as usize;
        Some(self.hit_of(self.geom.as_ref().map(|g| &g.1), s, i, None))
    }

    fn series(&self) -> Vec<String> {
        self.data.series.iter().map(|s| s.name.clone()).collect()
    }

    fn datum(&self, hit: &Hit) -> BTreeMap<String, Value> {
        self.data.datum(hit.index)
    }

    fn count(&self, view: &View, range: [f64; 2]) -> u64 {
        let n = self.data.len() as f64;
        let (a, b) = (range[0].min(range[1]).ceil().max(0.0), range[0].max(range[1]).floor().min(n - 1.0));
        let cats = if a <= b { (b - a + 1.0) as u64 } else { 0 };
        cats * self.shown(view).len() as u64
    }

    fn legend_at(&self, at: CellPos) -> Option<usize> {
        let (_, g) = self.geom.as_ref()?;
        g.legend.iter().find(|(_, r)| at.x >= r.x as f64 && at.x < (r.x + r.w) as f64 && at.y >= r.y as f64 && at.y < (r.y + 1) as f64).map(|(s, _)| *s)
    }
}

#[cfg(test)]
mod tests;
