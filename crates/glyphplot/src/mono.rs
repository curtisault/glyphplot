//! One ink: how a chart tells series apart without colour (REQ-MONO-010,
//! REQ-MONO-040, ADR-123).
//!
//! Line series 1 to 4 draw with the dash patterns solid, 3 on 1 off, 1 on 1
//! off, and 3 on 1 off 1 on 1 off, counted in dot columns from the x scale's
//! origin ([`origin`]), so a pan by `n` whole dots shifts the pattern by `n`
//! dots and the dashes never crawl. Series 5 to 8 repeat the four. Each
//! pattern has a one-cell legend key from the box-drawing block, `─`, `┄`,
//! `┈`, `╌`, drawn beside the series' name in the series' ink wherever the
//! series is drawn with its pattern; a series drawn solid (in colour, by
//! default) has the solid key `─`.

use crate::cells::Axis;

/// The dash patterns of line series 1 to 4, one entry a dot column: on or
/// off (REQ-MONO-010).
pub const PATTERNS: [&[bool]; 4] = [&[true], &[true, true, true, false], &[true, false], &[true, true, true, false, true, false]];

/// The legend keys of the patterns: `─` (U+2500), `┄` (U+2504), `┈` (U+2508)
/// and `╌` (U+254C) (REQ-MONO-040).
pub const KEYS: [char; 4] = ['\u{2500}', '\u{2504}', '\u{2508}', '\u{254C}'];

/// The pattern of series `index` (0 for the first).
pub fn pattern(index: usize) -> &'static [bool] {
    PATTERNS[index % PATTERNS.len()]
}

/// The legend key of series `index`.
pub fn key(index: usize) -> char {
    KEYS[index % KEYS.len()]
}

/// The dot column of the x scale's origin, counted from the axis's first
/// dot: the number of whole dots from 0 to the axis's low end, so a dot
/// column `k` of the plot is dot `k + origin(axis)` counted from 0. Panning by
/// whole dots moves it by as many (REQ-MONO-010). Held to ±2^52; 0 on a
/// degenerate axis.
pub fn origin(axis: &Axis) -> i64 {
    let dots = axis.dots();
    if axis.is_degenerate() || dots == 0 {
        return 0;
    }
    let per_dot = (axis.hi() - axis.lo()) / dots as f64;
    let d = (axis.lo() / per_dot).round();
    let limit = (1i64 << 52) as f64;
    if d >= limit {
        1 << 52
    } else if d <= -limit {
        -(1 << 52)
    } else {
        d as i64
    }
}

/// Whether series `index` lights dot column `k` of a plot whose x axis has
/// the origin offset `origin`.
pub fn lit(index: usize, k: i64, origin: i64) -> bool {
    let p = pattern(index);
    p[(k.wrapping_add(origin)).rem_euclid(p.len() as i64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-MONO-010: the four patterns, counted from the x origin, so a pan
    /// by whole dots carries the pattern with the data.
    #[test]
    fn dash_patterns_follow_the_origin() {
        let row = |i: usize, o: i64| -> String { (0..12).map(|k| if lit(i, k, o) { '#' } else { '.' }).collect() };
        assert_eq!(row(0, 0), "############");
        assert_eq!(row(1, 0), "###.###.###.");
        assert_eq!(row(2, 0), "#.#.#.#.#.#.");
        assert_eq!(row(3, 0), "###.#.###.#.");
        // An axis of 100 dots over 0..100: a dot a unit. Panned by 3 units it
        // starts 3 dots further on, and so does the pattern.
        let a = Axis::x(0.0, 100.0, 50, 2);
        let b = Axis::x(3.0, 103.0, 50, 2);
        assert_eq!((origin(&a), origin(&b)), (0, 3));
        for k in 0..40 {
            assert_eq!(lit(1, k, origin(&b)), lit(1, k + 3, origin(&a)), "dot {k}");
        }
        assert_eq!(origin(&Axis::x(1.0, 1.0, 10, 2)), 0);
        assert_eq!(KEYS.iter().collect::<std::collections::BTreeSet<_>>().len(), 4);
    }
}
