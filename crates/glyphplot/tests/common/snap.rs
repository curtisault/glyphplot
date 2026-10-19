//! The snapshot format and writer (REQ-SNAP-010, REQ-SNAP-020, ADR-129).
//!
//! A file `tests/snapshots/<type>/<fixture>.snap` (or
//! `<mode>-<fixture>.snap` for a non-default mode) is UTF-8 text:
//!
//! - a head line: `# glyphplot snapshot 1 · <type> · <fixture> · mode <mode> · options <json> · default view`;
//! - then blocks, each opened by a line starting `@ `: `@ <cols>x<rows> · t=<t> · <env>`, plus
//!   ` · <what was done>` for a block after input.
//!
//! A whole block follows its `@` line with the frame's rows, each between
//! two `|`, then the line `ink` and one line of runs a row, then the line
//! `back` and one line of runs a row. A run is a palette index as two
//! lowercase hex digits (`..` for `NO_BACK` in the back layer), followed by
//! its length in decimal when it is more than one, runs separated by one
//! space: `3a12 05 ..40`.
//!
//! A hash block is its `@` line ending ` · fnv1a <16 hex>`: FNV-1a 64 over the
//! cols and rows as `u32` little-endian, then every glyph as `u32`
//! little-endian, then every ink, then every back, each row-major.
//!
//! What each file holds, chosen so that it stays reviewable (about 13 KB a
//! file for a braille chart at 80 × 24):
//!
//! - whole: 80 × 24 at t = 0 in colour; `META.min` at t = 0 in colour; 80 × 24
//!   at t = 0 in colour after a seeded zoom (a ctrl+wheel of three lines up and
//!   a wheel end at a seeded cell, then a move to another seeded cell, which
//!   shows the crosshair and readout);
//! - hashes: 80 × 24 at t = 1, 2.5 and 6 in colour; 80 × 24 at t = 0 in one
//!   ink and on paper; 320 × 120 at t = 0 in colour.

use std::fmt::Write as _;

use glyphplot::grid::{Grid, NO_BACK};

/// The format's version, in every head line.
pub const VERSION: u32 = 1;

/// The FNV-1a 64 hash of a frame, as the module says.
pub fn fnv1a(g: &Grid) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for v in [g.cols() as u32, g.rows() as u32] {
        v.to_le_bytes().into_iter().for_each(&mut eat);
    }
    for &c in g.glyphs() {
        c.to_le_bytes().into_iter().for_each(&mut eat);
    }
    g.inks().iter().copied().for_each(&mut eat);
    g.backs().iter().copied().for_each(&mut eat);
    h
}

/// One row of a layer as runs (REQ-SNAP-020); `back` spells `NO_BACK` `..`.
pub fn runs(row: &[u8], back: bool) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < row.len() {
        let v = row[i];
        let n = row[i..].iter().take_while(|&&w| w == v).count();
        if !out.is_empty() {
            out.push(' ');
        }
        if back && v == NO_BACK {
            out.push_str("..");
        } else {
            let _ = write!(out, "{v:02x}");
        }
        if n > 1 {
            let _ = write!(out, "{n}");
        }
        i += n;
    }
    out
}

/// The `@` line's label of a frame.
pub fn label(g: &Grid, t: f64, env: &str, what: Option<&str>) -> String {
    let mut s = format!("@ {}x{} · t={t} · {env}", g.cols(), g.rows());
    if let Some(w) = what {
        s += " · ";
        s += w;
    }
    s
}

/// A whole block: the label, the rows, the ink layer and the back layer.
pub fn whole(out: &mut String, g: &Grid, t: f64, env: &str, what: Option<&str>) {
    out.push_str(&label(g, t, env, what));
    out.push('\n');
    for row in g.text().split('\n') {
        out.push('|');
        out.push_str(row);
        out.push_str("|\n");
    }
    out.push_str("ink\n");
    for row in g.inks().chunks_exact(g.cols()) {
        out.push_str(&runs(row, false));
        out.push('\n');
    }
    out.push_str("back\n");
    for row in g.backs().chunks_exact(g.cols()) {
        out.push_str(&runs(row, true));
        out.push('\n');
    }
}

/// A hash block: the label and the frame's FNV-1a hash.
pub fn hash(out: &mut String, g: &Grid, t: f64, env: &str, what: Option<&str>) {
    let _ = writeln!(out, "{} · fnv1a {:016x}", label(g, t, env, what), fnv1a(g));
}

/// Where two snapshot texts first differ, in words: the line, the block it
/// is in, and both lines.
pub fn first_difference(want: &str, got: &str) -> String {
    let (w, g): (Vec<&str>, Vec<&str>) = (want.lines().collect(), got.lines().collect());
    let n = w.iter().zip(&g).take_while(|(a, b)| a == b).count();
    let block = g[..n.min(g.len())].iter().rev().find(|l| l.starts_with("@ ")).copied().unwrap_or("the head");
    format!("line {} (in {block}): file {:?}, frame {:?}", n + 1, w.get(n).copied().unwrap_or("<end>"), g.get(n).copied().unwrap_or("<end>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_read_as_the_spec_writes_them() {
        let mut row = vec![0x3a; 12];
        row.push(5);
        row.extend([NO_BACK; 40]);
        assert_eq!(runs(&row, true), "3a12 05 ..40");
        assert_eq!(runs(&[255, 255], false), "ff2");
    }
}
