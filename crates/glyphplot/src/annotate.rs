//! Annotations: `hline`, `vline`, `band`, `label` and `marker`, in data
//! coordinates, clipped to their pane (REQ-ANNOT-010).
//!
//! An [`Annotation`] is one entry of the `annotations` option as
//! `spec/schemas/options.schema.json` shapes it; [`parse`] reads the option.
//! Layout places them last (`layout::layout`, step 7) with [`place`], which
//! turns each into [`Mark`]s in grid cells and hides every tick label an
//! annotation's axis label overlaps; [`draw`] paints the marks at the
//! annotations stage of a [`Frame`] (REQ-TEXT-020).
//!
//! Where each goes:
//!
//! - `hline`: a braille rule at the value's dot row across its scale's pane,
//!   and its label in a y gutter (`axis`, else its scale's side), on the
//!   value's row, where tick labels go. Layout widens the gutter to hold it.
//! - `vline`: a braille rule at the value's dot column through every pane,
//!   and its label on the x axis's label row, centred on the value's column
//!   and pushed inside the axis.
//! - `band`: on the `x` scale, the columns from `from` to `to` in every
//!   pane; on a y scale, those rows of its pane. Shaded on the `rule` colour
//!   behind the cells, or with `░` on empty cells in one ink; its label at the
//!   band's top left cell inside the plot.
//! - `label`: the text from the point's cell, cut with `…` at the plot's
//!   right edge.
//! - `marker`: the glyph (default `●`) at the point's cell, its label two
//!   cells to its right.
//!
//! A `label` or `marker` without `scale` takes the first pane's first y
//! scale. A mark whose value falls outside its pane's domain, or whose scale
//! no axis binds, is left out: nothing is drawn outside its pane. Marks
//! never take a cell a label holds (REQ-TEXT-010): rules, glyphs and shades
//! skip such cells, and annotation text is placed as a label, refused whole
//! when it would overlap one.

use serde_json::{Map, Value};

use crate::canvas::Glyphs;
use crate::cells::Axis;
use crate::chart::MakeError;
use crate::grid::{Grid, BLANK};
use crate::layout::{Layout, PlacedAxis, Rect, Side, TickLabel, X_DOTS, Y_DOTS};
use crate::palette::{ACCENT, INK, MUTED, RULE};
use crate::text::{self, stage, Frame};

/// `░`, a band's shade in one ink.
const SHADE: u32 = 0x2591;

/// The gutter an `hline`'s label goes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gutter {
    /// The left y gutter.
    Left,
    /// The right y gutter.
    Right,
}

/// A marker annotation's glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkerGlyph {
    /// `▲` (U+25B2).
    Up,
    /// `▼` (U+25BC).
    Down,
    /// `●` (U+25CF), the default.
    #[default]
    Dot,
    /// `◆` (U+25C6).
    Diamond,
}

impl MarkerGlyph {
    /// The glyph's character.
    pub fn char(self) -> char {
        match self {
            MarkerGlyph::Up => '\u{25B2}',
            MarkerGlyph::Down => '\u{25BC}',
            MarkerGlyph::Dot => '\u{25CF}',
            MarkerGlyph::Diamond => '\u{25C6}',
        }
    }
}

/// One annotation, as `options.schema.json`'s `$defs/annotation` shapes it
/// (REQ-ANNOT-010). Values are in data units (epoch ms on a time scale).
#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    /// A horizontal line at a y value across its pane.
    Hline {
        /// The y scale the value is in.
        scale: String,
        /// The value.
        value: f64,
        /// Text drawn on the axis, hiding the tick labels it overlaps.
        label: Option<String>,
        /// The gutter the label goes in; default its scale's side.
        axis: Option<Gutter>,
    },
    /// A vertical line at an x value through every pane.
    Vline {
        /// The x scale; default `x`.
        scale: Option<String>,
        /// The value.
        value: f64,
        /// Text drawn on the x axis, hiding the tick labels it overlaps.
        label: Option<String>,
    },
    /// A range shaded: vertical on the x scale, horizontal on a y scale.
    Band {
        /// The scale the range is in.
        scale: String,
        /// The range's start.
        from: f64,
        /// The range's end.
        to: f64,
        /// Text drawn at the band's top left.
        label: Option<String>,
    },
    /// Text at a point.
    Label {
        /// The y scale `y` is in; default the first pane's first.
        scale: Option<String>,
        /// The point's x.
        x: f64,
        /// The point's y.
        y: f64,
        /// The text.
        text: String,
    },
    /// A glyph at a point.
    Marker {
        /// The y scale `y` is in; default the first pane's first.
        scale: Option<String>,
        /// The point's x.
        x: f64,
        /// The point's y.
        y: f64,
        /// The glyph; default `●`.
        glyph: Option<MarkerGlyph>,
        /// Text drawn beside the glyph.
        label: Option<String>,
    },
}

/// The `annotations` option read: a list of annotations, none when absent
/// or null. Anything the schema refuses (not a list, an unknown type or key,
/// a missing or mistyped field, an empty `label` text, a glyph or gutter not
/// listed) is an error naming the option and the annotation's index. Read by
/// hand rather than through serde's derive, whose internally tagged enum
/// cost about 16 KB gzipped of the line module (`mise run size`).
pub fn parse(value: Option<&Value>) -> Result<Vec<Annotation>, MakeError> {
    let err = |message: String| MakeError::Option { key: "annotations".into(), message };
    let list = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(list)) => list,
        Some(_) => return Err(err("expected a list of annotations".into())),
    };
    list.iter().enumerate().map(|(i, v)| one(v).map_err(|e| err(format!("annotation {i}: {e}")))).collect()
}

/// The fields of one annotation's object, read with the schema's types.
struct Fields<'a>(&'a Map<String, Value>);

impl Fields<'_> {
    fn num(&self, key: &str) -> Result<f64, String> {
        self.0.get(key).and_then(Value::as_f64).ok_or_else(|| format!("`{key}` must be a number"))
    }

    fn opt_str(&self, key: &str) -> Result<Option<String>, String> {
        match self.0.get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(format!("`{key}` must be a string")),
        }
    }

    fn str(&self, key: &str) -> Result<String, String> {
        self.opt_str(key)?.ok_or_else(|| format!("`{key}` must be a string"))
    }
}

/// One annotation from its JSON object.
fn one(v: &Value) -> Result<Annotation, String> {
    let o = v.as_object().ok_or("expected an object")?;
    let kind = o.get("type").and_then(Value::as_str).unwrap_or("");
    let keys: &[&str] = match kind {
        "hline" => &["type", "scale", "value", "label", "axis"],
        "vline" => &["type", "scale", "value", "label"],
        "band" => &["type", "scale", "from", "to", "label"],
        "label" => &["type", "scale", "x", "y", "text"],
        "marker" => &["type", "scale", "x", "y", "glyph", "label"],
        _ => return Err("`type` must be one of hline, vline, band, label, marker".into()),
    };
    if let Some(k) = o.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(format!("{kind} has no key `{k}`"));
    }
    let f = Fields(o);
    Ok(match kind {
        "hline" => Annotation::Hline {
            scale: f.str("scale")?,
            value: f.num("value")?,
            label: f.opt_str("label")?,
            axis: match f.opt_str("axis")?.as_deref() {
                None => None,
                Some("left") => Some(Gutter::Left),
                Some("right") => Some(Gutter::Right),
                Some(_) => return Err("`axis` must be left or right".into()),
            },
        },
        "vline" => Annotation::Vline { scale: f.opt_str("scale")?, value: f.num("value")?, label: f.opt_str("label")? },
        "band" => Annotation::Band { scale: f.str("scale")?, from: f.num("from")?, to: f.num("to")?, label: f.opt_str("label")? },
        "label" => {
            let text = f.str("text")?;
            if text.is_empty() {
                return Err("a label's text is empty".into());
            }
            Annotation::Label { scale: f.opt_str("scale")?, x: f.num("x")?, y: f.num("y")?, text }
        }
        _ => Annotation::Marker {
            scale: f.opt_str("scale")?,
            x: f.num("x")?,
            y: f.num("y")?,
            glyph: match f.opt_str("glyph")?.as_deref() {
                None => None,
                Some("\u{25B2}") => Some(MarkerGlyph::Up),
                Some("\u{25BC}") => Some(MarkerGlyph::Down),
                Some("\u{25CF}") => Some(MarkerGlyph::Dot),
                Some("\u{25C6}") => Some(MarkerGlyph::Diamond),
                Some(_) => return Err("`glyph` must be one of \u{25B2} \u{25BC} \u{25CF} \u{25C6}".into()),
            },
            label: f.opt_str("label")?,
        },
    })
}

/// What an annotation draws, in grid cells, as [`place`] lays it out.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// A horizontal braille rule along the `w` cells from `(x, y)`, at dot
    /// row `dot` (0 to 3) of each.
    HRule {
        /// The first column.
        x: usize,
        /// The row.
        y: usize,
        /// The cells along.
        w: usize,
        /// The dot row in the cell, from its top.
        dot: usize,
    },
    /// A vertical braille rule down the `h` cells from `(x, y)`, at dot
    /// column `dot` (0 or 1) of each.
    VRule {
        /// The column.
        x: usize,
        /// The first row.
        y: usize,
        /// The cells down.
        h: usize,
        /// The dot column in the cell, from its left.
        dot: usize,
    },
    /// A band's cells.
    Shade(Rect),
    /// A glyph in one cell.
    Glyph {
        /// The column.
        x: usize,
        /// The row.
        y: usize,
        /// The glyph.
        glyph: char,
    },
    /// Text from a cell, drawn as a label.
    Text {
        /// The first column.
        x: usize,
        /// The row.
        y: usize,
        /// The text, already cut to its room.
        text: String,
        /// Its ink: `accent` on an axis, `ink` in the plot.
        ink: u8,
    },
}

/// Where a scale is bound: the x scale, or a pane's y scale on a side.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Bound {
    X,
    Y { pane: usize, side: Side, domain: [f64; 2] },
}

/// The scale named `name` in `layout`, or the first pane's first y scale for
/// None.
fn find(layout: &Layout, name: Option<&str>) -> Option<Bound> {
    match name {
        Some(n) if n == layout.x_name => Some(Bound::X),
        Some(n) => layout.panes.iter().enumerate().find_map(|(i, p)| {
            if p.left_name.as_deref() == Some(n) {
                p.left_domain.map(|d| Bound::Y { pane: i, side: Side::Left, domain: d })
            } else if p.right_name.as_deref() == Some(n) {
                p.right_domain.map(|d| Bound::Y { pane: i, side: Side::Right, domain: d })
            } else {
                None
            }
        }),
        None => {
            let p = layout.panes.first()?;
            match (p.left_domain, p.right_domain) {
                (Some(d), _) => Some(Bound::Y { pane: 0, side: Side::Left, domain: d }),
                (None, Some(d)) => Some(Bound::Y { pane: 0, side: Side::Right, domain: d }),
                (None, None) => None,
            }
        }
    }
}

/// The x mapping over a plot's columns.
fn x_axis(layout: &Layout, plot: Rect) -> Axis {
    let [a, b] = layout.x_domain;
    Axis::x(a.min(b), a.max(b), plot.w, X_DOTS)
}

/// The y mapping over a plot's rows.
fn y_axis(domain: [f64; 2], plot: Rect) -> Axis {
    Axis::y(domain[0].min(domain[1]), domain[0].max(domain[1]), plot.h, Y_DOTS)
}

/// The `[first, last]` cells along `axis` a range covers inside its domain,
/// clipped to it; None when the range misses the domain or is not finite.
fn span(axis: &Axis, from: f64, to: f64) -> Option<(usize, usize)> {
    let (lo, hi) = (from.min(to), from.max(to));
    if !(lo.is_finite() && hi.is_finite()) || hi < axis.lo() || lo > axis.hi() {
        return None;
    }
    let a = axis.cell_in(lo.max(axis.lo()))?;
    let b = axis.cell_in(hi.min(axis.hi()))?;
    Some((a.min(b), a.max(b)))
}

/// The widest `hline` label each y gutter holds, `[left, right]` in cells:
/// what layout widens the gutters to (step 5). A label counts on a side
/// whose pane has an axis drawn there (`axes`, by pane: left, right), as
/// [`place`] puts it; `names` gives each pane's scale names.
pub fn gutter_labels(annotations: &[Annotation], names: &[[Option<&str>; 2]], axes: &[[bool; 2]]) -> [usize; 2] {
    let mut widest = [0usize; 2];
    for a in annotations {
        let Annotation::Hline { scale, label: Some(label), axis, .. } = a else { continue };
        let Some((pane, side)) = names.iter().enumerate().find_map(|(i, n)| {
            if n[0] == Some(scale.as_str()) {
                Some((i, 0))
            } else if n[1] == Some(scale.as_str()) {
                Some((i, 1))
            } else {
                None
            }
        }) else {
            continue;
        };
        let side = match axis {
            Some(Gutter::Left) => 0,
            Some(Gutter::Right) => 1,
            None => side,
        };
        if axes.get(pane).is_some_and(|a| a[side]) {
            widest[side] = widest[side].max(text::width(label));
        }
    }
    widest
}

/// Hides the tick labels of `axis` any cell of which is in `cells` (columns
/// `cells.0..cells.1` of grid row `row`), where the axis draws its labels.
fn hide_overlapped(axis: &mut PlacedAxis, row: usize, cells: (usize, usize)) {
    let r = axis.rect;
    for t in &mut axis.ticks {
        let Some(l) = &t.label else { continue };
        let (x, y) = label_cell(axis.side, r, l);
        if y == row && x < cells.1 && cells.0 < x + l.width {
            t.label = None;
        }
    }
}

/// The grid cell a tick label of an axis starts at, as `text::Frame::axis_labels`
/// draws it.
fn label_cell(side: Side, r: Rect, l: &TickLabel) -> (usize, usize) {
    match side {
        Side::Bottom => (r.x + l.start, r.y + 1),
        Side::Left => ((r.x + r.w).saturating_sub(1 + l.width), r.y + l.start),
        Side::Right => (r.x + 1, r.y + l.start),
    }
}

/// Lays the annotations out over `layout` (layout's step 7,
/// REQ-ANNOT-010): the marks each draws, in grid cells, clipped to their
/// pane, in the annotations' order; every tick label an axis label overlaps
/// is hidden in `layout`. `gutters` says whether the left and right gutters
/// keep their labels (REQ-LAYOUT-030 may drop them); a dropped gutter takes
/// no annotation label either. Nothing is placed when the layout is not
/// decorated.
pub fn place(layout: &mut Layout, annotations: &[Annotation], gutters: [bool; 2]) -> Vec<Mark> {
    let mut marks = Vec::new();
    if !layout.decorated {
        return marks;
    }
    for a in annotations {
        match a {
            Annotation::Hline { scale, value, label, axis } => {
                let Some(Bound::Y { pane, side, domain }) = find(layout, Some(scale.as_str())) else { continue };
                let plot = layout.panes[pane].plot;
                let ya = y_axis(domain, plot);
                let Some(dot) = ya.dot_in(*value) else { continue };
                let row = plot.y + dot / Y_DOTS;
                marks.push(Mark::HRule { x: plot.x, y: row, w: plot.w, dot: dot % Y_DOTS });
                let Some(label) = label else { continue };
                let side = match axis {
                    Some(Gutter::Left) => Side::Left,
                    Some(Gutter::Right) => Side::Right,
                    None => side,
                };
                let keep = if side == Side::Left { gutters[0] } else { gutters[1] };
                let p = &mut layout.panes[pane];
                let Some(placed) = (if side == Side::Left { p.left.as_mut() } else { p.right.as_mut() }) else { continue };
                if !keep || placed.rect.w < 2 {
                    continue;
                }
                let r = placed.rect;
                let text = text::truncate(label, r.w - 1);
                let w = text::width(&text);
                let x = if side == Side::Left { r.x + r.w - 1 - w } else { r.x + 1 };
                hide_overlapped(placed, row, (x, x + w));
                marks.push(Mark::Text { x, y: row, text, ink: ACCENT });
            }
            Annotation::Vline { scale, value, label } => {
                if !matches!(find(layout, Some(scale.as_deref().unwrap_or(&layout.x_name))), Some(Bound::X)) {
                    continue;
                }
                let mut col = None;
                for p in &layout.panes {
                    let Some(dot) = x_axis(layout, p.plot).dot_in(*value) else { continue };
                    col = Some(dot / X_DOTS);
                    marks.push(Mark::VRule { x: p.plot.x + dot / X_DOTS, y: p.plot.y, h: p.plot.h, dot: dot % X_DOTS });
                }
                let (Some(col), Some(label), Some(xa)) = (col, label, layout.x_axis.as_mut()) else { continue };
                let r = xa.rect;
                if r.h < 2 || r.w == 0 {
                    continue;
                }
                let text = text::truncate(label, r.w);
                let w = text::width(&text);
                let start = col.saturating_sub(w.saturating_sub(1) / 2).min(r.w - w);
                let (x, y) = (r.x + start, r.y + 1);
                hide_overlapped(xa, y, (x, x + w));
                marks.push(Mark::Text { x, y, text, ink: ACCENT });
            }
            Annotation::Band { scale, from, to, label } => {
                let rects: Vec<Rect> = match find(layout, Some(scale.as_str())) {
                    Some(Bound::X) => layout
                        .panes
                        .iter()
                        .filter_map(|p| span(&x_axis(layout, p.plot), *from, *to).map(|(a, b)| Rect::new(p.plot.x + a, p.plot.y, b - a + 1, p.plot.h)))
                        .collect(),
                    Some(Bound::Y { pane, domain, .. }) => {
                        let plot = layout.panes[pane].plot;
                        span(&y_axis(domain, plot), *from, *to).map(|(a, b)| Rect::new(plot.x, plot.y + a, plot.w, b - a + 1)).into_iter().collect()
                    }
                    None => Vec::new(),
                };
                let first = rects.first().copied();
                marks.extend(rects.into_iter().filter(|r| !r.is_empty()).map(Mark::Shade));
                if let (Some(r), Some(label)) = (first, label) {
                    let plot = layout.panes.iter().find(|p| p.plot.overlaps(&r)).map_or(r, |p| p.plot);
                    let room = plot.x + plot.w - r.x;
                    marks.push(Mark::Text { x: r.x, y: r.y, text: text::truncate(label, room), ink: INK });
                }
            }
            Annotation::Label { scale, x, y, text: t } => {
                if let Some((cx, cy, plot)) = point(layout, scale.as_deref(), *x, *y) {
                    marks.push(Mark::Text { x: cx, y: cy, text: text::truncate(t, plot.x + plot.w - cx), ink: INK });
                }
            }
            Annotation::Marker { scale, x, y, glyph, label } => {
                let Some((cx, cy, plot)) = point(layout, scale.as_deref(), *x, *y) else { continue };
                marks.push(Mark::Glyph { x: cx, y: cy, glyph: glyph.unwrap_or_default().char() });
                let room = (plot.x + plot.w).saturating_sub(cx + 2);
                if let (Some(label), true) = (label, room > 0) {
                    marks.push(Mark::Text { x: cx + 2, y: cy, text: text::truncate(label, room), ink: INK });
                }
            }
        }
    }
    marks
}

/// The grid cell of a point on a y scale (None: the first pane's first) and
/// its pane's plot, when the point is inside that pane's domains.
fn point(layout: &Layout, scale: Option<&str>, x: f64, y: f64) -> Option<(usize, usize, Rect)> {
    let Some(Bound::Y { pane, domain, .. }) = find(layout, scale) else { return None };
    let plot = layout.panes[pane].plot;
    let cx = x_axis(layout, plot).cell_in(x)?;
    let cy = y_axis(domain, plot).cell_in(y)?;
    Some((plot.x + cx, plot.y + cy, plot))
}

/// Gives a cell a rule's dots: a blank cell becomes the braille `mask` in
/// `ink`; a cell holding a glyph keeps it and takes `ink`, as the crosshair
/// does; a label's cell is left alone (REQ-TEXT-010).
fn rule_cell(frame: &mut Frame<'_, stage::Annotations>, x: usize, y: usize, mask: u8, ink: u8) {
    if frame.is_label(x, y) {
        return;
    }
    let grid: &mut Grid = frame.annotations();
    if x >= grid.cols() || y >= grid.rows() {
        return;
    }
    let glyph = if grid.glyph(x, y) == BLANK { Glyphs::Braille.glyph(mask) } else { grid.glyph(x, y) };
    grid.set(x as i32, y as i32, glyph, ink);
}

/// Paints placed marks at the annotations stage (REQ-ANNOT-010,
/// REQ-TEXT-020), in the order bands, rules, glyphs, text: rules and glyphs
/// in `accent`; bands on the `rule` colour behind their cells, or in one ink
/// (`mono`) with `░` in `muted` on blank cells; text as labels. No mark takes
/// a cell a label holds.
pub fn draw(frame: &mut Frame<'_, stage::Annotations>, marks: &[Mark], mono: bool) {
    for m in marks {
        let Mark::Shade(r) = m else { continue };
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                if frame.is_label(x, y) {
                    continue;
                }
                let grid = frame.annotations();
                if x >= grid.cols() || y >= grid.rows() {
                    continue;
                }
                if mono {
                    if grid.glyph(x, y) == BLANK {
                        grid.set(x as i32, y as i32, SHADE, MUTED);
                    }
                } else {
                    grid.set_back(x as i32, y as i32, RULE);
                }
            }
        }
    }
    for m in marks {
        match *m {
            Mark::HRule { x, y, w, dot } => {
                let mask = 0b11u8 << (dot * X_DOTS);
                for c in x..x + w {
                    rule_cell(frame, c, y, mask, ACCENT);
                }
            }
            Mark::VRule { x, y, h, dot } => {
                let mask = (0..Y_DOTS).fold(0u8, |m, row| m | 1 << (row * X_DOTS + dot));
                for r in y..y + h {
                    rule_cell(frame, x, r, mask, ACCENT);
                }
            }
            _ => {}
        }
    }
    for m in marks {
        if let Mark::Glyph { x, y, glyph } = *m {
            let grid = frame.grid();
            if !frame.is_label(x, y) && x < grid.cols() && y < grid.rows() {
                frame.annotations().set(x as i32, y as i32, glyph as u32, ACCENT);
            }
        }
    }
    for m in marks {
        if let Mark::Text { x, y, text, ink } = m {
            frame.label(*x, *y, text, *ink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::NO_BACK;
    use crate::layout::{layout, AxisInput, LayoutExtents, LayoutInput, PaneExtents, PaneInput};
    use crate::scale::{AxisFormat, Extents, Kind};

    fn ext(lo: f64, hi: f64) -> Extents {
        Extents { data: Some([lo, hi]), ..Extents::default() }
    }

    /// Two panes, `price` over 0..100 with `vol` on the right, and `volume`
    /// under it over 0..10, all under an x of 0..100.
    fn two_panes(annotations: Vec<Annotation>) -> (LayoutInput, LayoutExtents) {
        let mut input = LayoutInput::new(AxisInput::named(Kind::Linear, "x"), AxisInput::named(Kind::Linear, "price"));
        input.panes[0].weight = 3;
        input.panes[0].right = Some(AxisInput::named(Kind::Linear, "alt"));
        input.panes.push(PaneInput { weight: 1, left: Some(AxisInput::named(Kind::Linear, "volume")), right: None });
        input.annotations = annotations;
        let e = LayoutExtents {
            x: ext(0.0, 100.0),
            panes: vec![PaneExtents { left: ext(0.0, 100.0), right: ext(-1.0, 1.0) }, PaneExtents { left: ext(0.0, 10.0), ..PaneExtents::default() }],
        };
        (input, e)
    }

    fn json(text: &str) -> Vec<Annotation> {
        parse(Some(&serde_json::from_str(text).unwrap())).unwrap()
    }

    /// Paints a layout's annotations over an empty frame.
    fn paint(l: &Layout, mono: bool) -> Grid {
        let mut grid = Grid::new(l.cols, l.rows);
        let mut f = Frame::new(&mut grid, NO_BACK).then::<stage::Axes>();
        if let Some(a) = &l.x_axis {
            f.axis(a);
        }
        let mut f = f.then::<stage::Labels>();
        for p in &l.panes {
            for a in p.left.iter().chain(p.right.iter()) {
                f.axis_labels(a);
            }
        }
        if let Some(a) = &l.x_axis {
            f.axis_labels(a);
        }
        let mut f = f.then::<stage::Annotations>();
        draw(&mut f, &l.marks, mono);
        grid
    }

    /// The options parse as the schema shapes them, and what it refuses is
    /// an error naming the option.
    #[test]
    fn annotations_parse_as_the_schema_shapes_them() {
        let a = json(
            r#"[{"type":"hline","scale":"price","value":101.5,"label":"last","axis":"right"},
                {"type":"vline","value":3},
                {"type":"band","scale":"x","from":1,"to":2,"label":"b"},
                {"type":"label","x":1,"y":2,"text":"hi"},
                {"type":"marker","x":1,"y":2,"glyph":"▲"}]"#,
        );
        assert_eq!(a.len(), 5);
        assert_eq!(a[0], Annotation::Hline { scale: "price".into(), value: 101.5, label: Some("last".into()), axis: Some(Gutter::Right) });
        assert!(matches!(a[4], Annotation::Marker { glyph: Some(MarkerGlyph::Up), .. }));
        assert!(parse(None).unwrap().is_empty());
        for bad in [
            r#"[{"type":"circle"}]"#,
            r#"[{"type":"hline","value":1}]"#,
            r#"[{"type":"vline","value":1,"colour":"red"}]"#,
            r#"[{"type":"label","x":1,"y":2,"text":""}]"#,
            r#"[{"type":"marker","x":1,"y":2,"glyph":"x"}]"#,
            r#"{"type":"vline","value":1}"#,
        ] {
            let e = parse(Some(&serde_json::from_str(bad).unwrap())).unwrap_err();
            assert!(e.to_string().starts_with("option `annotations`: "), "{bad}: {e}");
        }
    }

    /// REQ-ANNOT-010: every kind draws inside its pane in data coordinates,
    /// and a value outside its pane's domain draws nothing.
    #[test]
    fn annotations_clip_to_their_pane() {
        let (input, e) = two_panes(json(
            r#"[{"type":"hline","scale":"price","value":50},
                {"type":"hline","scale":"volume","value":5},
                {"type":"hline","scale":"price","value":500},
                {"type":"vline","value":25},
                {"type":"vline","value":-5},
                {"type":"band","scale":"x","from":60,"to":1000},
                {"type":"band","scale":"volume","from":-3,"to":2},
                {"type":"label","x":90,"y":90,"text":"a long label that runs off the plot"},
                {"type":"marker","x":10,"y":20,"label":"m"},
                {"type":"marker","scale":"volume","x":10,"y":20}]"#,
        ));
        let l = layout(80, 24, &input, &e).unwrap();
        let (top, bottom) = (l.panes[0].plot, l.panes[1].plot);
        let inside = |p: Rect, x: usize, y: usize| x >= p.x && x < p.x + p.w && y >= p.y && y < p.y + p.h;
        let mut rules = 0;
        for m in &l.marks {
            match m {
                Mark::HRule { x, y, w, .. } => {
                    rules += 1;
                    let p = if inside(top, *x, *y) { top } else { bottom };
                    assert!(inside(p, *x, *y) && *x + *w == p.x + p.w, "{m:?} spans its pane");
                }
                Mark::VRule { x, y, h, .. } => {
                    let p = if inside(top, *x, *y) { top } else { bottom };
                    assert!(inside(p, *x, *y) && *y + *h == p.y + p.h, "{m:?} spans its pane");
                }
                Mark::Shade(r) => assert!(r.w > 0 && r.h > 0 && (top.overlaps(r) || bottom.overlaps(r))),
                Mark::Glyph { x, y, .. } => assert!(inside(top, *x, *y), "{m:?}"),
                Mark::Text { x, y, text, .. } => assert!(inside(top, *x, *y) && x + text::width(text) <= top.x + top.w, "{m:?} stays in the plot"),
            }
        }
        assert_eq!(rules, 2, "the hline at 500 is outside the price domain");
        let vrules = l.marks.iter().filter(|m| matches!(m, Mark::VRule { .. })).count();
        assert_eq!(vrules, 2, "the vline at 25 goes through both panes; the one at -5 through none");
        let shades: Vec<Rect> = l.marks.iter().filter_map(|m| if let Mark::Shade(r) = m { Some(*r) } else { None }).collect();
        assert_eq!(shades.len(), 3, "the x band in both panes, the volume band in its own");
        assert_eq!(shades[0].x + shades[0].w, top.x + top.w, "a band past the domain stops at the plot's edge");
        assert_eq!(shades[2].y + shades[2].h, bottom.y + bottom.h, "the volume band from below the domain stops at the bottom");
        assert!(!l.marks.iter().any(|m| matches!(m, Mark::Glyph { x, y, .. } if inside(bottom, *x, *y))), "the volume marker at 20 is above its domain");
        let cut = l.marks.iter().find_map(|m| if let Mark::Text { text, .. } = m { text.strip_suffix('\u{2026}') } else { None });
        assert!(cut.is_some(), "the long label is cut at the plot's edge");
        // Drawn: the vline's rule is braille in accent in both panes; the
        // band shades the back; in one ink, `░` instead.
        let g = paint(&l, false);
        let vx = l.marks.iter().find_map(|m| if let Mark::VRule { x, .. } = m { Some(*x) } else { None }).unwrap();
        assert_eq!((g.glyph(vx, top.y + 1) & !0xFF, g.ink(vx, top.y + 1)), (0x2800, ACCENT));
        assert_eq!(g.back(shades[2].x + 1, shades[2].y), RULE);
        let mono = paint(&l, true);
        assert_eq!(mono.glyph(shades[2].x + 1, shades[2].y), SHADE);
        assert_eq!(mono.back(shades[2].x + 1, shades[2].y), NO_BACK);
    }

    /// REQ-ANNOT-010: an annotation label on an axis hides the tick labels
    /// it overlaps, and only those; its gutter widens to hold it.
    #[test]
    fn axis_label_hides_overlapped_ticks() {
        let plain = two_panes(Vec::new());
        let before = layout(80, 24, &plain.0, &plain.1).unwrap();
        let left = before.panes[0].left.as_ref().unwrap();
        // Put an hline on the row of the second tick label, and a vline on
        // the x tick at 50.
        let tick = left.ticks.iter().filter(|t| t.label.is_some()).nth(1).unwrap().clone();
        assert!(before.x_axis.as_ref().unwrap().ticks.iter().any(|t| t.value == 50.0 && t.label.is_some()));
        let anns = vec![
            Annotation::Hline { scale: "price".into(), value: tick.value, label: Some("LAST 123.45".into()), axis: None },
            Annotation::Vline { scale: None, value: 50.0, label: Some("event".into()) },
        ];
        let (input, e) = two_panes(anns);
        let l = layout(80, 24, &input, &e).unwrap();
        let left = l.panes[0].left.as_ref().unwrap();
        assert_eq!(left.rect.w, "LAST 123.45".len() + 1, "the gutter holds the label");
        assert_eq!(l.panes[1].left.as_ref().unwrap().rect.w, left.rect.w, "gutters stay aligned across panes");
        let hidden: Vec<f64> = left.ticks.iter().filter(|t| t.label.is_none()).map(|t| t.value).collect();
        assert_eq!(hidden, [tick.value], "only the overlapped tick label is hidden");
        assert!(left.ticks.iter().any(|t| t.value == tick.value), "its mark stays");
        let x = l.x_axis.as_ref().unwrap();
        assert!(x.ticks.iter().find(|t| t.value == 50.0).unwrap().label.is_none(), "the x tick label under the vline's label is hidden");
        assert!(x.ticks.iter().filter(|t| t.value != 50.0).all(|t| t.label.is_some()), "the others stay");
        let g = paint(&l, false);
        let text = g.text();
        assert!(text.contains("LAST 123.45"), "{text}");
        assert!(text.lines().nth(x.rect.y + 1).unwrap().contains("event"), "{text}");
        let row = l.panes[0].plot.y + tick.cell;
        assert_eq!(g.ink(left.rect.x, row), ACCENT, "the label is in accent");
        // With the right gutter asked for, the left keeps its labels.
        let (mut input, e) =
            two_panes(vec![Annotation::Hline { scale: "price".into(), value: tick.value, label: Some("R".into()), axis: Some(Gutter::Right) }]);
        input.panes[0].right.as_mut().unwrap().format = AxisFormat::default();
        let l = layout(80, 24, &input, &e).unwrap();
        assert!(l.panes[0].left.as_ref().unwrap().ticks.iter().all(|t| t.label.is_some()));
        assert!(l.marks.iter().any(|m| matches!(m, Mark::Text { x, .. } if *x == l.panes[0].right.as_ref().unwrap().rect.x + 1)));
    }
}
