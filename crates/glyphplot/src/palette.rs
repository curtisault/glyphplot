//! The palette's fixed layout: 255 indices, theme tokens, a scale ramp and
//! reserved entries (REQ-COLOR-010, REQ-COLOR-020).
//!
//! A cell's ink and back are `u8` indices into this layout, never colours; the
//! shell looks each index up in the table that [`crate::theme::resolve`]
//! builds (REQ-COLOR-030).
//!
//! | Index | Token | Used for |
//! |---|---|---|
//! | 0 | `ink` | titles, tick labels, readouts |
//! | 1 | `muted` | axes, ticks |
//! | 2 | `rule` | gridlines |
//! | 3 | `accent` | crosshair, focus |
//! | 4 | `ground` | the chart's ground, when it has one |
//! | 5, 6 | `up`, `down` | candles, bars, signed values |
//! | 7 | `selection` | brushed ranges, selected data |
//! | 8–15 | `series-1` … `series-8` | series colours |
//! | 16–31 | reserved | |
//! | 32–159 | scale steps 0–127 | a continuous scale |
//! | 160–254 | reserved | a second scale, if a chart needs one |
//! | 255 | [`NO_BACK`] | no back colour |

/// The number of colour entries in a palette: indices 0–254 (REQ-COLOR-010).
pub const PALETTE_LEN: usize = 255;

/// Text: titles, tick labels and readouts; what a cleared cell's ink is (REQ-COLOR-020).
pub const INK: u8 = 0;
/// Axes and tick marks (REQ-COLOR-020).
pub const MUTED: u8 = 1;
/// Gridlines, a decoration exempt from the contrast rules (REQ-COLOR-020).
pub const RULE: u8 = 2;
/// The crosshair, the brush outline and the focus ring (REQ-COLOR-020).
pub const ACCENT: u8 = 3;
/// The chart's ground, when it has one (REQ-COLOR-020).
pub const GROUND: u8 = 4;
/// A rising candle or bar, and a positive signed value (REQ-COLOR-020).
pub const UP: u8 = 5;
/// A falling candle or bar, and a negative signed value (REQ-COLOR-020).
pub const DOWN: u8 = 6;
/// The fill behind a brushed range and selected data (REQ-COLOR-020).
pub const SELECTION: u8 = 7;
/// The first series colour, `series-1`; series 1–8 are indices 8–15 (REQ-COLOR-020).
pub const SERIES_START: u8 = 8;
/// The number of series colours; series past the eighth repeat them (REQ-COLOR-020).
pub const SERIES_COUNT: u8 = 8;
/// The first reserved token index; 16–31 are reserved (REQ-COLOR-020).
pub const TOKENS_RESERVED_START: u8 = 16;
/// The first index after the theme tokens, 0–31 (REQ-COLOR-020).
pub const TOKENS_END: u8 = 32;
/// The first step of the scale ramp, step 0 (REQ-COLOR-020).
pub const SCALE_START: u8 = 32;
/// The number of steps in the scale ramp, 32–159 (REQ-COLOR-020, REQ-COLOR-040).
pub const SCALE_STEPS: u8 = 128;
/// The first index after the scale ramp, where the second reserved block starts; 160–254 are reserved (REQ-COLOR-020).
pub const SCALE_END: u8 = 160;
/// The back index meaning no back colour; it is not a palette entry
/// (REQ-COLOR-010). The grid's [`crate::grid::NO_BACK`], re-exported.
pub const NO_BACK: u8 = crate::grid::NO_BACK;

/// The token names of indices 0–15, as the theme object and the custom
/// properties spell them (REQ-COLOR-020, REQ-THEME-010).
pub const TOKEN_NAMES: [&str; 16] = [
    "ink",
    "muted",
    "rule",
    "accent",
    "ground",
    "up",
    "down",
    "selection",
    "series-1",
    "series-2",
    "series-3",
    "series-4",
    "series-5",
    "series-6",
    "series-7",
    "series-8",
];

/// One palette entry: red, green, blue and alpha, each 0–255, alpha 255 opaque.
pub type Rgba = [u8; 4];

/// The fill of every reserved entry (16–31 and 160–254): transparent black,
/// so an index written there by mistake draws nothing (REQ-COLOR-020).
pub const RESERVED_FILL: Rgba = [0, 0, 0, 0];

/// The palette index of the series at `index`, counted from 0; series past
/// the eighth wrap round to the first (REQ-COLOR-020).
pub const fn series(index: usize) -> u8 {
    SERIES_START + (index % SERIES_COUNT as usize) as u8
}

/// The palette index of the scale step for `t`, where 0 is the scale's low end
/// and 1 its high end: `t` is split into 128 equal bins, values below 0 take
/// step 0 and values above 1 take step 127. NaN takes step 0 and never
/// panics; a chart leaves a missing value out before it gets here
/// (REQ-COLOR-020, REQ-COLOR-040).
pub fn scale_step(t: f64) -> u8 {
    let steps = f64::from(SCALE_STEPS);
    // NaN fails every comparison and lands on step 0.
    let step = if t >= 1.0 {
        SCALE_STEPS - 1
    } else if t > 0.0 {
        // `t * 128` lies in (0, 128) here, so the cast neither wraps nor saturates.
        (t * steps) as u8
    } else {
        0
    };
    SCALE_START + step
}

/// The palette index of scale step `step`, 0–127; a step past 127 takes 127 (REQ-COLOR-020).
pub const fn scale_index(step: u8) -> u8 {
    if step >= SCALE_STEPS {
        SCALE_START + SCALE_STEPS - 1
    } else {
        SCALE_START + step
    }
}

/// Whether `index` is a reserved entry, 16–31 or 160–254 (REQ-COLOR-020).
pub const fn is_reserved(index: u8) -> bool {
    (index >= TOKENS_RESERVED_START && index < TOKENS_END) || (index >= SCALE_END && index != NO_BACK)
}

/// The resolved colours of a palette: 255 RGBA entries in the fixed layout,
/// built by [`crate::theme::resolve`] (REQ-COLOR-010, REQ-COLOR-030).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    entries: [Rgba; PALETTE_LEN],
}

impl Palette {
    /// A palette whose every entry is [`RESERVED_FILL`], for the resolver to fill.
    pub(crate) const fn reserved() -> Palette {
        Palette { entries: [RESERVED_FILL; PALETTE_LEN] }
    }

    /// Sets entry `index`; [`NO_BACK`] is not an entry and is ignored.
    pub(crate) fn set(&mut self, index: u8, rgba: Rgba) {
        if let Some(entry) = self.entries.get_mut(usize::from(index)) {
            *entry = rgba;
        }
    }

    /// The colour at `index`, or `None` for [`NO_BACK`] (REQ-COLOR-010).
    pub fn get(&self, index: u8) -> Option<Rgba> {
        self.entries.get(usize::from(index)).copied()
    }

    /// The 255 entries, in index order.
    pub fn entries(&self) -> &[Rgba; PALETTE_LEN] {
        &self.entries
    }

    /// The entries as 1,020 bytes, RGBA by RGBA in index order, as the
    /// boundary hands them to the shell (REQ-COLOR-030).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.entries.iter().flatten().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_constants_are_fixed() {
        assert_eq!([INK, MUTED, RULE, ACCENT, GROUND, UP, DOWN, SELECTION], [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!((0..8).map(series).collect::<Vec<_>>(), (8..16).collect::<Vec<_>>());
        assert_eq!((TOKENS_RESERVED_START, TOKENS_END), (16, 32));
        assert_eq!((SCALE_START, SCALE_STEPS, SCALE_END), (32, 128, 160));
        assert_eq!(usize::from(SCALE_START) + usize::from(SCALE_STEPS), usize::from(SCALE_END));
        assert_eq!(NO_BACK, 255);
        assert_eq!(PALETTE_LEN, 255);
        assert_eq!(TOKEN_NAMES[usize::from(INK)], "ink");
        assert_eq!(TOKEN_NAMES[usize::from(SELECTION)], "selection");
        assert_eq!(TOKEN_NAMES[usize::from(series(7))], "series-8");
        let reserved: Vec<u8> = (0..=255).filter(|&i| is_reserved(i)).collect();
        let expected: Vec<u8> = (16..32).chain(160..255).collect();
        assert_eq!(reserved, expected);
    }

    #[test]
    fn series_wraps_over_eight() {
        assert_eq!(series(0), 8);
        assert_eq!(series(7), 15);
        assert_eq!(series(8), 8);
        assert_eq!(series(19), 11);
        assert_eq!(series(usize::MAX), SERIES_START + (usize::MAX % 8) as u8);
    }

    #[test]
    fn scale_step_covers_the_ramp_and_survives_nan() {
        assert_eq!(scale_step(0.0), 32);
        assert_eq!(scale_step(1.0), 159);
        assert_eq!(scale_step(0.5), 32 + 64);
        assert_eq!(scale_step(1.0 / 128.0 - 1e-12), 32);
        assert_eq!(scale_step(1.0 / 128.0), 33);
        assert_eq!(scale_step(1.0 - 1e-12), 159);
        assert_eq!(scale_step(-3.0), 32);
        assert_eq!(scale_step(7.0), 159);
        assert_eq!(scale_step(f64::NAN), 32);
        assert_eq!(scale_step(f64::INFINITY), 159);
        assert_eq!(scale_step(f64::NEG_INFINITY), 32);
        assert_eq!(scale_index(0), 32);
        assert_eq!(scale_index(127), 159);
        assert_eq!(scale_index(200), 159);
    }

    #[test]
    fn palette_get_has_no_entry_for_no_back() {
        let mut palette = Palette::reserved();
        palette.set(INK, [1, 2, 3, 255]);
        palette.set(NO_BACK, [9, 9, 9, 9]);
        assert_eq!(palette.get(INK), Some([1, 2, 3, 255]));
        assert_eq!(palette.get(254), Some(RESERVED_FILL));
        assert_eq!(palette.get(NO_BACK), None);
        assert_eq!(palette.to_bytes().len(), 1020);
    }

    proptest::proptest! {
        #[test]
        fn scale_step_always_lands_in_the_ramp(t in proptest::num::f64::ANY) {
            let index = scale_step(t);
            proptest::prop_assert!((SCALE_START..SCALE_END).contains(&index));
        }
    }
}
