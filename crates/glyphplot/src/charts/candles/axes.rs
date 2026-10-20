//! The candlestick chart's axes, fitted after `layout::layout` has placed
//! the title, the panes and the x axis's rows.
//!
//! Each pane's y axis is the chart's own: under 20 rows (or always, with
//! `snap: "rows"`) every tick sits on a row with a nice step a row
//! (REQ-TICK-050, `ticks::snap_rows`); otherwise the domain is niced unless
//! the data would fill under three quarters of it, and the ticks are fitted
//! by lowering the count (ADR-111). Ticks and labels are chosen in the
//! column's displayed units (REQ-DATA-100) and placed as stored values.
//! Then the left gutter is the widest label plus the line's cell, dropped
//! when it would take more than a third of the columns (REQ-LAYOUT-030),
//! every plot spans the columns right of it, and the x axis is ticked over
//! the plot's width by the scale the chart gives (an index time scale,
//! REQ-SCALE-060).

use crate::cells::Axis;
use crate::format::{step_format, Display, Locale, Specifier};
use crate::layout::{Layout, Tick, TickLabel, PANE_ROWS, Y_DOTS, Y_GAP, Y_ROWS_PER_TICK};
use crate::text;
use crate::ticks::{mark, snap_rows, tick_step, ticks};

/// The least share of the niced domain the data must span for the nice one
/// to be kept, as the line chart keeps it.
const NICE_FILL: f64 = 0.75;

/// A y axis as fitted: its stored domain, low to high, its ticks and the
/// step between them in displayed units.
#[derive(Clone, Debug, PartialEq)]
pub struct YAxis {
    /// The domain in stored units.
    pub domain: [f64; 2],
    /// The ticks, values stored.
    pub ticks: Vec<Tick>,
    /// The displayed step between ticks.
    pub step: f64,
}

/// What a y axis is fitted from.
pub struct YInput<'a> {
    /// The data's least and greatest stored value.
    pub extent: [f64; 2],
    /// The rows of the pane.
    pub rows: usize,
    /// Whether ticks snap to rows (REQ-TICK-050).
    pub snap: bool,
    /// Whether 0 is in the domain (volume).
    pub zero: bool,
    /// The column's display transform (REQ-DATA-100).
    pub display: &'a Display,
    /// The labels' format.
    pub spec: &'a Specifier,
    /// The locale labels are written in.
    pub locale: &'a Locale,
}

/// A domain of no span widened as `scale::resolve_domain` widens one: by 1
/// each way around 0, a tenth of the value otherwise.
fn widen([a, b]: [f64; 2]) -> [f64; 2] {
    if a < b {
        [a, b]
    } else if a == 0.0 {
        [-1.0, 1.0]
    } else {
        [a - a.abs() / 10.0, a + a.abs() / 10.0]
    }
}

/// The ticks of `labels` placed on `axis` (displayed values), with their
/// stored values, and whether every label fits a row apart.
fn placed(axis: &Axis, labels: Vec<(f64, String)>, display: &Display) -> (Vec<Tick>, bool) {
    let mut out: Vec<Tick> = Vec::with_capacity(labels.len());
    for (v, text) in labels {
        let Some((cell, dot)) = mark(axis, v) else { continue };
        let width = text::width(&text);
        out.push(Tick { value: display.stored(v), cell, dot, label: Some(TickLabel { text, start: cell, width }) });
    }
    let fits = out.windows(2).all(|w| w[0].cell > w[1].cell + Y_GAP);
    (out, fits)
}

/// The y axis of a pane (REQ-TICK-050, REQ-DATA-100).
pub fn y_axis(input: &YInput) -> YAxis {
    let YInput { extent, rows, snap, zero, display, spec, locale } = *input;
    let shown = display.domain(extent);
    let mut d = [shown[0].min(shown[1]), shown[0].max(shown[1])];
    if zero {
        d = [d[0].min(0.0), d[1].max(0.0)];
    }
    let d = widen(d);
    let stored = |[a, b]: [f64; 2]| {
        let (x, y) = (display.stored(a), display.stored(b));
        [x.min(y), x.max(y)]
    };
    let count = (rows / Y_ROWS_PER_TICK).max(1) as f64;
    if snap {
        if let Some(rs) = snap_rows(d[0], d[1], rows.max(1)) {
            // `snap_rows` puts the data at the bottom rows; the rows it
            // leaves are split above and below, at the same step.
            let s = rs.step;
            let spare = rows as f64 - ((d[1] / s - 0.5).ceil() - (d[0] / s + 0.5).floor() + 1.0);
            let rs = if spare >= 2.0 { snap_rows(d[0] - (spare / 2.0).floor() * s, d[1], rows).filter(|r| r.step == s).unwrap_or(rs) } else { rs };
            let axis = Axis::y(rs.domain[0], rs.domain[1], rows, Y_DOTS);
            let f = rs.tick_format(count, spec, locale);
            let labels = rs.tick_rows(count).into_iter().rev().map(|r| rs.row_value(r)).map(|v| (v, f.format(v))).collect();
            let (ticks, _) = placed(&axis, labels, display);
            let step = match ticks.as_slice() {
                [a, b, ..] => (display.shown(b.value) - display.shown(a.value)).abs(),
                _ => rs.step,
            };
            return YAxis { domain: stored(rs.domain), ticks, step };
        }
    }
    let niced = crate::scale::nice(d[0], d[1], count);
    let d = if (d[1] - d[0]) < NICE_FILL * (niced[1] - niced[0]) { d } else { niced };
    let axis = Axis::y(d[0], d[1], rows, Y_DOTS);
    let mut fallback = Vec::new();
    let mut step = tick_step(d[0], d[1], count);
    for c in (1..=count as usize).rev() {
        let c = c as f64;
        let f = step_format(tick_step(d[0], d[1], c), d[0], d[1], spec, locale);
        let labels = ticks(d[0], d[1], c).into_iter().map(|v| (v, f.format(v))).collect();
        let (t, fits) = placed(&axis, labels, display);
        step = tick_step(d[0], d[1], c).abs();
        if fits && !t.is_empty() {
            return YAxis { domain: stored(d), ticks: t, step };
        }
        if !t.is_empty() {
            fallback = t;
        }
    }
    for t in &mut fallback {
        t.label = None;
    }
    YAxis { domain: stored(d), ticks: fallback, step }
}

/// Sets each pane's y axis and domain, then the left gutter, the plots'
/// columns and the x axis's ticks over the plot's width (REQ-LAYOUT-030).
pub fn finish(lay: &mut Layout, ys: &[Option<YAxis>], x_ticks: impl FnOnce(usize) -> Vec<Tick>) {
    let cols = lay.cols;
    for (p, y) in lay.panes.iter_mut().zip(ys) {
        let Some(y) = y else { continue };
        p.left_domain = Some(y.domain);
        if let Some(a) = &mut p.left {
            a.domain = y.domain;
            a.ticks = y.ticks.clone();
            if p.plot.h < PANE_ROWS {
                a.ticks.iter_mut().for_each(|t| t.label = None);
            }
        }
    }
    let widest = |lay: &Layout| -> Option<usize> {
        let axes: Vec<_> = lay.panes.iter().filter_map(|p| p.left.as_ref()).collect();
        (!axes.is_empty()).then(|| axes.iter().flat_map(|a| a.ticks.iter()).filter_map(|t| t.label.as_ref()).map(|l| l.width).max().unwrap_or(0))
    };
    let mut left_w = widest(lay).map_or(0, |w| w + 1);
    if 3 * left_w > cols {
        for a in lay.panes.iter_mut().filter_map(|p| p.left.as_mut()) {
            a.ticks.iter_mut().for_each(|t| t.label = None);
        }
        left_w = widest(lay).map_or(0, |w| w + 1);
    }
    let plot_w = cols.saturating_sub(left_w);
    for p in &mut lay.panes {
        p.plot.x = left_w;
        p.plot.w = plot_w;
        if let Some(a) = &mut p.left {
            a.rect.x = 0;
            a.rect.w = left_w;
        }
    }
    if let Some(x) = &mut lay.x_axis {
        x.rect.x = left_w;
        x.rect.w = plot_w;
        x.ticks = if plot_w > 0 { x_ticks(plot_w) } else { Vec::new() };
        if x.rect.h < 2 {
            x.ticks.iter_mut().for_each(|t| t.label = None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fit(extent: [f64; 2], rows: usize, snap: bool, display: &Display) -> YAxis {
        let spec = Specifier::default();
        let locale = Locale::default();
        y_axis(&YInput { extent, rows, snap, zero: false, display, spec: &spec, locale: &locale })
    }

    /// REQ-TICK-050: snapped, every tick is a row's centre and the step a
    /// row is nice; unsnapped, labels keep a row between them.
    #[test]
    fn snapped_ticks_sit_on_rows() {
        let y = fit([61.3, 128.9], 16, true, &Display::default());
        assert_eq!(y.domain, [57.5, 137.5]);
        let rows: Vec<usize> = y.ticks.iter().map(|t| t.cell).collect();
        assert!(rows.windows(2).all(|w| w[0] > w[1] + 1), "{rows:?}");
        let axis = Axis::y(y.domain[0], y.domain[1], 16, Y_DOTS);
        for t in &y.ticks {
            assert_eq!(axis.position(t.value), t.cell as f64 + 0.5, "{} sits on its row's centre", t.value);
        }
        // Data in the bottom rows of a pane is centred, the spare rows split.
        let y = fit([60.0, 104.0], 16, true, &Display::default());
        assert_eq!(y.domain, [42.5, 122.5]);
        let y = fit([61.3, 128.9], 30, false, &Display::default());
        assert!(y.ticks.windows(2).all(|w| w[0].cell > w[1].cell + Y_GAP));
    }

    /// REQ-DATA-100: labels are in displayed units, values stored.
    #[test]
    fn labels_are_displayed_values() {
        let cents = Display::new(Some(0.01), None, None);
        let y = fit([6130.0, 12890.0], 16, true, &cents);
        let labels: Vec<&str> = y.ticks.iter().filter_map(|t| t.label.as_ref()).map(|l| l.text.as_str()).collect();
        assert!(labels.contains(&"100"), "{labels:?}");
        assert!(y.ticks.iter().any(|t| (t.value - 10_000.0).abs() < 1e-9));
    }
}
