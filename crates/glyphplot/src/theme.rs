//! Themes: a theme object resolved to the palette's 255 RGBA entries
//! (REQ-COLOR-030, REQ-COLOR-040).
//!
//! A [`Theme`] holds the tokens of `spec/schemas/theme.schema.json`, each one
//! optional. [`resolve`] is the one function that turns it into a
//! [`Palette`]: a token left out falls back to the dark or the light default
//! set ([`DARK`], [`LIGHT`]), chosen by [`Theme::base`]. The scale ramp is the
//! theme's scale stops interpolated in Oklab over 128 steps.

use std::fmt;

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::palette::{self, Palette, Rgba};

/// A colour as the theme object writes it, `#rrggbb`: opaque sRGB, 8 bits a
/// channel (theme.schema.json `$defs/color`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red, 0–255, sRGB-encoded.
    pub r: u8,
    /// Green, 0–255, sRGB-encoded.
    pub g: u8,
    /// Blue, 0–255, sRGB-encoded.
    pub b: u8,
}

impl Color {
    /// The colour whose `#rrggbb` is `0xrrggbb`.
    pub const fn hex(rgb: u32) -> Color {
        Color { r: (rgb >> 16) as u8, g: (rgb >> 8) as u8, b: rgb as u8 }
    }

    /// The colour written `#rrggbb` (either case), or `None` for any other
    /// form; `#fff`, names and `rgb()` are not colours here (theme.schema.json).
    pub fn parse(text: &str) -> Option<Color> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(digits, 16).ok().map(Color::hex)
    }

    /// The colour as an opaque palette entry.
    pub const fn rgba(self) -> Rgba {
        [self.r, self.g, self.b, 255]
    }

    /// The luma `0.2126 r + 0.7152 g + 0.0722 b` of the encoded channels,
    /// 0–255, that picks the dark or the light set (REQ-THEME-020).
    pub fn luma(self) -> f64 {
        0.2126 * f64::from(self.r) + 0.7152 * f64::from(self.g) + 0.0722 * f64::from(self.b)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// A theme object (`spec/schemas/theme.schema.json`): every token optional,
/// each one left out falling back to the default set [`Theme::base`] picks
/// (REQ-COLOR-030).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Theme {
    /// The colour behind the chart.
    pub ground: Option<Color>,
    /// Text: titles, tick labels, readouts.
    pub ink: Option<Color>,
    /// Axes and tick marks.
    pub muted: Option<Color>,
    /// Gridlines.
    pub rule: Option<Color>,
    /// The crosshair, the brush outline and the focus ring.
    pub accent: Option<Color>,
    /// The fill behind a brushed range and selected data.
    pub selection: Option<Color>,
    /// A rising candle or bar.
    pub up: Option<Color>,
    /// A falling candle or bar.
    pub down: Option<Color>,
    /// Series colours 1 to 8, in order; fewer than eight replace the first ones only.
    pub series: Option<Vec<Color>>,
    /// The sequential scale's stops, low end first; at least two.
    pub scale: Option<Vec<Color>>,
    /// The diverging scale's stops, low end first; at least three.
    pub diverging: Option<Vec<Color>>,
    /// Paper: dark marks on a light ground; picks the light default set.
    pub paper: Option<bool>,
}

/// Which default set fills a theme's missing tokens (REQ-THEME-020).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    /// The dark set, [`DARK`].
    Dark,
    /// The light set, [`LIGHT`].
    Light,
}

impl Base {
    /// The default set this base names.
    pub const fn defaults(self) -> &'static Defaults {
        match self {
            Base::Dark => &DARK,
            Base::Light => &LIGHT,
        }
    }
}

/// Why a theme object was refused: the field at fault and what was wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeError {
    /// The field at fault, as the object spells it (`ink`, `series[2]`), or
    /// `theme` when the fault is the whole object.
    pub field: String,
    /// What was wrong with it.
    pub message: String,
}

impl ThemeError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> ThemeError {
        ThemeError { field: field.into(), message: message.into() }
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "theme `{}`: {}", self.field, self.message)
    }
}

impl std::error::Error for ThemeError {}

/// Every field the theme object may hold.
const FIELDS: &str = "ground, ink, muted, rule, accent, selection, up, down, series, scale, diverging, paper";

impl Theme {
    /// The theme in a JSON string, or an error naming the field at fault. It
    /// never panics, whatever the input (REQ-COLOR-030).
    pub fn from_json(json: &str) -> Result<Theme, ThemeError> {
        let value: Value = serde_json::from_str(json).map_err(|e| ThemeError::new("theme", format!("not JSON: {e}")))?;
        Theme::from_value(&value)
    }

    /// The theme in a parsed JSON value, checked against theme.schema.json, or
    /// an error naming the field at fault.
    pub fn from_value(value: &Value) -> Result<Theme, ThemeError> {
        let object = value.as_object().ok_or_else(|| ThemeError::new("theme", format!("expected an object, got {}", kind(value))))?;
        let mut theme = Theme::default();
        for (key, field) in object {
            match key.as_str() {
                "ground" => theme.ground = Some(color(key, field)?),
                "ink" => theme.ink = Some(color(key, field)?),
                "muted" => theme.muted = Some(color(key, field)?),
                "rule" => theme.rule = Some(color(key, field)?),
                "accent" => theme.accent = Some(color(key, field)?),
                "selection" => theme.selection = Some(color(key, field)?),
                "up" => theme.up = Some(color(key, field)?),
                "down" => theme.down = Some(color(key, field)?),
                "series" => theme.series = Some(colors(key, field, 1, Some(8))?),
                "scale" => theme.scale = Some(colors(key, field, 2, None)?),
                "diverging" => theme.diverging = Some(colors(key, field, 3, None)?),
                "paper" => theme.paper = Some(field.as_bool().ok_or_else(|| ThemeError::new(key, format!("expected true or false, got {}", kind(field))))?),
                _ => return Err(ThemeError::new(key.clone(), format!("unknown field; expected one of {FIELDS}"))),
            }
        }
        Ok(theme)
    }

    /// The default set that fills this theme's missing tokens: the light set
    /// for paper; otherwise by the ground's luma (under 128 dark); otherwise
    /// by the ink's luma (under 128 means dark text, so light); otherwise dark
    /// (REQ-THEME-020).
    pub fn base(&self) -> Base {
        let dark_if = |dark: bool| if dark { Base::Dark } else { Base::Light };
        if self.paper == Some(true) {
            Base::Light
        } else if let Some(ground) = self.ground {
            dark_if(ground.luma() < 128.0)
        } else if let Some(ink) = self.ink {
            dark_if(ink.luma() >= 128.0)
        } else {
            Base::Dark
        }
    }

    /// The colour of a token by its name in [`palette::TOKEN_NAMES`], with
    /// the default set's colour where the theme leaves it out.
    fn token(&self, index: u8, defaults: &Defaults) -> Color {
        let own = match index {
            palette::INK => self.ink,
            palette::MUTED => self.muted,
            palette::RULE => self.rule,
            palette::ACCENT => self.accent,
            palette::GROUND => self.ground,
            palette::UP => self.up,
            palette::DOWN => self.down,
            palette::SELECTION => self.selection,
            _ => None,
        };
        own.unwrap_or_else(|| defaults.token(index))
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Theme, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Theme::from_value(&value).map_err(serde::de::Error::custom)
    }
}

/// What kind of JSON value `value` is, for error messages.
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The colour in field `name`, or an error naming it.
fn color(name: &str, value: &Value) -> Result<Color, ThemeError> {
    match value {
        Value::String(text) => Color::parse(text).ok_or_else(|| ThemeError::new(name, format!("expected a colour as #rrggbb, got {text:?}"))),
        other => Err(ThemeError::new(name, format!("expected a colour as #rrggbb, got {}", kind(other)))),
    }
}

/// The colours in array field `name`, at least `min` and at most `max`.
fn colors(name: &str, value: &Value, min: usize, max: Option<usize>) -> Result<Vec<Color>, ThemeError> {
    let items = value.as_array().ok_or_else(|| ThemeError::new(name, format!("expected an array of colours, got {}", kind(value))))?;
    if items.len() < min || max.is_some_and(|max| items.len() > max) {
        let bound = match max {
            Some(max) => format!("{min} to {max}"),
            None => format!("at least {min}"),
        };
        return Err(ThemeError::new(name, format!("expected {bound} colours, got {}", items.len())));
    }
    items.iter().enumerate().map(|(i, item)| color(&format!("{name}[{i}]"), item)).collect()
}

/// A default set: every token, eight series and the scales, so a theme's
/// missing tokens always have a colour (REQ-THEME-020, REQ-THEME-040).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Defaults {
    /// The colour behind the chart.
    pub ground: Color,
    /// Text: at least 4.5:1 against the ground.
    pub ink: Color,
    /// Axes and ticks: at least 4.5:1 against the ground.
    pub muted: Color,
    /// Gridlines: decoration, exempt from contrast.
    pub rule: Color,
    /// Crosshair and focus: at least 3:1 against the ground.
    pub accent: Color,
    /// The fill behind a brushed range.
    pub selection: Color,
    /// A rising candle or bar: at least 3:1 against the ground.
    pub up: Color,
    /// A falling candle or bar: at least 3:1 against the ground.
    pub down: Color,
    /// Series 1–8: each at least 3:1 against the ground.
    pub series: [Color; 8],
    /// The sequential scale's stops, low end (the ground's side) first.
    pub scale: &'static [Color],
    /// The diverging scale's stops, middle on the ground's side.
    pub diverging: &'static [Color],
}

impl Defaults {
    /// The colour of token `index`, 0–15 (ink to series 8); black past them.
    pub fn token(&self, index: u8) -> Color {
        match index {
            palette::INK => self.ink,
            palette::MUTED => self.muted,
            palette::RULE => self.rule,
            palette::ACCENT => self.accent,
            palette::GROUND => self.ground,
            palette::UP => self.up,
            palette::DOWN => self.down,
            palette::SELECTION => self.selection,
            i => self.series.get(usize::from(i.wrapping_sub(palette::SERIES_START))).copied().unwrap_or(Color::hex(0)),
        }
    }
}

/// Cividis sampled at nine evenly spaced points (matplotlib 3.10.0
/// `colormaps["cividis"]`, `to_hex`), dark end first.
const CIVIDIS: [Color; 9] = [
    Color::hex(0x00224e),
    Color::hex(0x1a386f),
    Color::hex(0x434e6c),
    Color::hex(0x61656f),
    Color::hex(0x7d7c78),
    Color::hex(0x9b9476),
    Color::hex(0xbcae6c),
    Color::hex(0xdec958),
    Color::hex(0xfee838),
];

/// [`CIVIDIS`] light end first, for a light ground.
const CIVIDIS_REVERSED: [Color; 9] = [CIVIDIS[8], CIVIDIS[7], CIVIDIS[6], CIVIDIS[5], CIVIDIS[4], CIVIDIS[3], CIVIDIS[2], CIVIDIS[1], CIVIDIS[0]];

/// The blue–dark grey–orange diverging scale for a dark ground
/// (`discovery/bench/a11y-theming/results-palettes.md`, "glyphplot div dark").
const DIVERGING_DARK: [Color; 9] = [
    Color::hex(0x90cbfe),
    Color::hex(0x74a6d0),
    Color::hex(0x62819c),
    Color::hex(0x505e6b),
    Color::hex(0x3d3d3d),
    Color::hex(0x695949),
    Color::hex(0x987554),
    Color::hex(0xca935c),
    Color::hex(0xfeb263),
];

/// The blue–light grey–orange diverging scale for a light ground
/// (`discovery/bench/a11y-theming/results-palettes.md`, "glyphplot div light").
const DIVERGING_LIGHT: [Color; 9] = [
    Color::hex(0x0c54a0),
    Color::hex(0x4d7bb6),
    Color::hex(0x83a2cb),
    Color::hex(0xb9cadf),
    Color::hex(0xf2f2f2),
    Color::hex(0xe0c6b8),
    Color::hex(0xcc9b80),
    Color::hex(0xb77148),
    Color::hex(0x9a490c),
];

/// The dark default set. Provisional: the owner picks by eye (the plan's open
/// question 10). Ground, ink, muted and rule are the tokens measured in
/// `discovery/bench/a11y-theming/results-palettes.md` ("Theme text and rule
/// tokens"). The series are the candidate "anchored to Okabe-Ito, dark
/// grounds" in `results-anchored.md`. Up and down are Okabe–Ito's sky blue
/// and orange, the blue/orange row of `results-updown.md`. The accent is
/// series 2; the selection is the example theme's (`spec/schemas/examples/theme.json`).
/// The scale is cividis, dark end first; the diverging scale is
/// "glyphplot div dark" from `results-palettes.md`.
pub const DARK: Defaults = Defaults {
    ground: Color::hex(0x131518),
    ink: Color::hex(0xe8ebef),
    muted: Color::hex(0x8c95a1),
    rule: Color::hex(0x262a2f),
    accent: Color::hex(0x629cfb),
    selection: Color::hex(0x24344f),
    up: Color::hex(0x56b4e9),
    down: Color::hex(0xe69f00),
    series: [
        Color::hex(0xb29a02),
        Color::hex(0x629cfb),
        Color::hex(0x72916d),
        Color::hex(0xc7f226),
        Color::hex(0xabe6fd),
        Color::hex(0xf8d6b2),
        Color::hex(0xe424a4),
        Color::hex(0xb3b3b2),
    ],
    scale: &CIVIDIS,
    diverging: &DIVERGING_DARK,
};

/// The light default set. Provisional: the owner picks by eye (the plan's
/// open question 10). Ground, ink, muted and rule are the tokens measured in
/// `discovery/bench/a11y-theming/results-palettes.md` ("Theme text and rule
/// tokens"). The series are the candidate "anchored to Okabe-Ito, light
/// grounds" in `results-anchored.md`. Up is series 2 and down a dark orange
/// chosen for this set, since `results-updown.md`'s blue/orange pair is under
/// 3:1 on a light ground; the accent is series 2 and the selection a pale
/// blue. The scale is cividis, light end first; the diverging scale is
/// "glyphplot div light" from `results-palettes.md`.
pub const LIGHT: Defaults = Defaults {
    ground: Color::hex(0xf5f6f7),
    ink: Color::hex(0x1e2125),
    muted: Color::hex(0x5f6873),
    rule: Color::hex(0xdfe2e6),
    accent: Color::hex(0x0784ff),
    selection: Color::hex(0xd3e1f7),
    up: Color::hex(0x0784ff),
    down: Color::hex(0xb35c00),
    series: [
        Color::hex(0x9a8b0a),
        Color::hex(0x0784ff),
        Color::hex(0x628463),
        Color::hex(0x746100),
        Color::hex(0x324268),
        Color::hex(0x6c2b28),
        Color::hex(0xd2569b),
        Color::hex(0x5f6165),
    ],
    scale: &CIVIDIS_REVERSED,
    diverging: &DIVERGING_LIGHT,
};

/// Resolves a theme to the palette's 255 RGBA entries in the fixed layout,
/// the one function the shell, the CLI and the exports call (REQ-COLOR-020,
/// REQ-COLOR-030). Tokens 0–15 are the theme's, each missing one taken from
/// the default set of [`Theme::base`] (a missing ground included: whether a
/// chart paints its ground is the shell's choice). Series given replace the
/// first ones only. Indices 32–159 are the scale ramp of the theme's scale
/// stops, or the default set's when it has fewer than two. Reserved entries,
/// 16–31 and 160–254, are [`palette::RESERVED_FILL`], transparent. Every
/// token is opaque.
pub fn resolve(theme: &Theme) -> Palette {
    let defaults = theme.base().defaults();
    let mut out = Palette::reserved();
    for index in palette::INK..=palette::SELECTION {
        out.set(index, theme.token(index, defaults).rgba());
    }
    let own_series = theme.series.as_deref().unwrap_or(&[]);
    for (i, default) in defaults.series.iter().enumerate() {
        out.set(palette::series(i), own_series.get(i).unwrap_or(default).rgba());
    }
    let stops = match theme.scale.as_deref() {
        Some(stops) if stops.len() >= 2 => stops,
        _ => defaults.scale,
    };
    for (step, colour) in scale_ramp(stops).iter().enumerate() {
        out.set(palette::SCALE_START + step as u8, colour.rgba());
    }
    out
}

/// Resolves the theme in a JSON string, as the boundary's
/// `resolve_palette(theme_json)` does; errors name the field at fault and
/// nothing panics (REQ-COLOR-030).
pub fn resolve_json(json: &str) -> Result<Palette, ThemeError> {
    Theme::from_json(json).map(|theme| resolve(&theme))
}

/// The least contrast a series colour keeps against the ground
/// (REQ-THEME-070, ADR-123).
pub const SERIES_CONTRAST: f64 = 3.0;

/// What a debug build logs about a theme (REQ-THEME-070): one warning for
/// each series colour the theme sets under 3:1 against its resolved ground,
/// naming the token and the ratio, as `series-2 #e8e8e8 is 1.21:1 against the
/// ground #f5f6f7; a series colour needs 3:1`. The colour is still used; the
/// shell logs each line once.
pub fn warnings(theme: &Theme) -> Vec<String> {
    let ground = theme.token(palette::GROUND, theme.base().defaults());
    let two = crate::format::Specifier::parse("ratio", ".2f").map(|s| crate::format::NumberFormat::new(&s, &crate::format::Locale::default()));
    let mut out = Vec::new();
    for (i, &c) in theme.series.as_deref().unwrap_or(&[]).iter().enumerate() {
        let ratio = oklab::contrast(c, ground);
        if ratio < SERIES_CONTRAST {
            let ratio = two.as_ref().map_or_else(|_| String::from("under 3"), |f| f.format(ratio));
            out.push(format!("series-{} {c} is {ratio}:1 against the ground {ground}; a series colour needs 3:1", i + 1));
        }
    }
    out
}

/// The 128 steps of a scale: `stops` evenly spaced, low end first, and
/// interpolated in Oklab, step 0 the first stop and step 127 the last
/// (REQ-COLOR-040). One stop gives a flat ramp; none gives black.
pub fn scale_ramp(stops: &[Color]) -> [Color; palette::SCALE_STEPS as usize] {
    let mut ramp = [Color::hex(0); palette::SCALE_STEPS as usize];
    let labs: Vec<[f64; 3]> = stops.iter().map(|&c| oklab::from_srgb(c)).collect();
    let (Some(first), Some(last)) = (labs.first(), labs.last()) else {
        return ramp;
    };
    let segments = labs.len() - 1;
    let last_step = f64::from(palette::SCALE_STEPS - 1);
    for (step, slot) in ramp.iter_mut().enumerate() {
        let position = step as f64 / last_step * segments as f64;
        // `position` lies in [0, segments]; the last step sits on the last stop.
        let segment = (position as usize).min(segments.saturating_sub(1));
        let (from, to) = match (labs.get(segment), labs.get(segment + 1)) {
            (Some(from), Some(to)) => (from, to),
            _ => (first, last),
        };
        let f = position - segment as f64;
        let mixed = [from[0] + (to[0] - from[0]) * f, from[1] + (to[1] - from[1]) * f, from[2] + (to[2] - from[2]) * f];
        *slot = oklab::to_srgb(mixed);
    }
    ramp
}

/// sRGB, linear sRGB, Oklab and CIELAB, from their published definitions.
///
/// Oklab is Björn Ottosson's, "A perceptual color space for image
/// processing" (2020, <https://bottosson.github.io/posts/oklab/>), from linear
/// sRGB. The sRGB transfer function is IEC 61966-2-1's. CIELAB is CIE 15's,
/// D65 white, from linear sRGB by the sRGB primaries.
pub mod oklab {
    use super::Color;

    /// An sRGB-encoded channel, 0–1, to linear light.
    pub fn to_linear(c: f64) -> f64 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// A linear channel, 0–1, to sRGB encoding.
    pub fn to_encoded(c: f64) -> f64 {
        if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    }

    /// The colour's linear sRGB channels, 0–1.
    pub fn linear(c: Color) -> [f64; 3] {
        [to_linear(f64::from(c.r) / 255.0), to_linear(f64::from(c.g) / 255.0), to_linear(f64::from(c.b) / 255.0)]
    }

    /// The colour nearest linear sRGB `rgb`: each channel clipped to 0–1,
    /// encoded and rounded to 8 bits. NaN gives 0.
    pub fn from_linear(rgb: [f64; 3]) -> Color {
        let byte = |c: f64| (to_encoded(clip_unit(c)) * 255.0).round() as u8;
        Color { r: byte(rgb[0]), g: byte(rgb[1]), b: byte(rgb[2]) }
    }

    /// `c` clipped to 0–1, written out since `f64::clamp` is refused
    /// (REQ-LINT-040); NaN fails both comparisons and gives 0.
    pub fn clip_unit(c: f64) -> f64 {
        if c >= 1.0 {
            1.0
        } else if c > 0.0 {
            c
        } else {
            0.0
        }
    }

    /// Linear sRGB to Oklab `[L, a, b]` (Ottosson 2020).
    pub fn from_linear_srgb([r, g, b]: [f64; 3]) -> [f64; 3] {
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        [
            0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
            1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
            0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
        ]
    }

    /// Oklab `[L, a, b]` to linear sRGB, unclipped (Ottosson 2020).
    pub fn to_linear_srgb([lightness, a, b]: [f64; 3]) -> [f64; 3] {
        let l = lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
        let m = lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
        let s = lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
        let (l, m, s) = (l * l * l, m * m * m, s * s * s);
        [
            4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
            -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
            -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
        ]
    }

    /// An sRGB colour to Oklab.
    pub fn from_srgb(c: Color) -> [f64; 3] {
        from_linear_srgb(linear(c))
    }

    /// Oklab to the nearest 8-bit sRGB colour, out-of-gamut channels clipped.
    pub fn to_srgb(lab: [f64; 3]) -> Color {
        from_linear(to_linear_srgb(lab))
    }

    /// Linear sRGB to CIELAB `[L*, a*, b*]`, D65 white (CIE 15).
    pub fn cielab_from_linear([r, g, b]: [f64; 3]) -> [f64; 3] {
        let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
        let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
        let z = (0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b) / 1.088_83;
        let f = |t: f64| {
            let delta: f64 = 6.0 / 29.0;
            if t > delta * delta * delta {
                t.cbrt()
            } else {
                t / (3.0 * delta * delta) + 4.0 / 29.0
            }
        };
        let (fx, fy, fz) = (f(x), f(y), f(z));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    /// An sRGB colour to CIELAB.
    pub fn cielab(c: Color) -> [f64; 3] {
        cielab_from_linear(linear(c))
    }

    /// The CIE 1976 colour difference ΔE76: the distance in CIELAB.
    pub fn delta_e76(a: Color, b: Color) -> f64 {
        let (p, q) = (cielab(a), cielab(b));
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    }

    /// The WCAG 2.2 relative luminance of a colour, 0–1.
    pub fn relative_luminance(c: Color) -> f64 {
        let [r, g, b] = linear(c);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// The WCAG 2.2 contrast ratio of two colours, 1–21.
    pub fn contrast(a: Color, b: Color) -> f64 {
        let (x, y) = (relative_luminance(a), relative_luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::oklab::{cielab, cielab_from_linear, contrast, delta_e76, from_srgb, linear};
    use super::*;
    use crate::palette::{NO_BACK, RESERVED_FILL, SCALE_END, SCALE_START, TOKENS_END, TOKENS_RESERVED_START};

    /// The colour tokens of the theme object, in palette order.
    const COLOR_TOKENS: [&str; 8] = ["ink", "muted", "rule", "accent", "ground", "up", "down", "selection"];

    fn c(rgb: u32) -> Color {
        Color::hex(rgb)
    }

    #[test]
    fn palette_layout_is_fixed() {
        // Every token a distinct colour, so each one's index shows.
        let theme = Theme {
            ink: Some(c(0x000001)),
            muted: Some(c(0x000002)),
            rule: Some(c(0x000003)),
            accent: Some(c(0x000004)),
            ground: Some(c(0x000005)),
            up: Some(c(0x000006)),
            down: Some(c(0x000007)),
            selection: Some(c(0x000008)),
            series: Some((9..17).map(c).collect()),
            scale: Some(vec![c(0x102030), c(0xf0e0d0)]),
            ..Theme::default()
        };
        let resolved = resolve(&theme);
        for index in 0..16u8 {
            assert_eq!(resolved.get(index), Some([0, 0, index + 1, 255]), "token {}", palette::TOKEN_NAMES[usize::from(index)]);
        }
        for index in (TOKENS_RESERVED_START..TOKENS_END).chain(SCALE_END..NO_BACK) {
            assert_eq!(resolved.get(index), Some(RESERVED_FILL), "reserved {index}");
        }
        assert_eq!(resolved.get(SCALE_START), Some([0x10, 0x20, 0x30, 255]));
        assert_eq!(resolved.get(SCALE_END - 1), Some([0xf0, 0xe0, 0xd0, 255]));
        for index in SCALE_START..SCALE_END {
            assert_eq!(resolved.get(index).map(|e| e[3]), Some(255), "scale step {index} is opaque");
        }
        assert_eq!(resolved.get(NO_BACK), None);
    }

    #[test]
    fn resolve_gives_255_entries() {
        for theme in [Theme::default(), Theme { paper: Some(true), ..Theme::default() }, Theme::from_json(EXAMPLE).unwrap()] {
            let resolved = resolve(&theme);
            assert_eq!(resolved.entries().len(), 255);
            assert_eq!(resolved.to_bytes().len(), 255 * 4);
        }
        // The defaults fill every token of an empty theme.
        let dark = resolve(&Theme::default());
        assert_eq!(dark.get(palette::GROUND), Some(DARK.ground.rgba()));
        assert_eq!(dark.get(palette::series(7)), Some(DARK.series[7].rgba()));
        // Fewer than eight series replace the first ones only.
        let two = resolve(&Theme { series: Some(vec![c(0x010203), c(0x040506)]), ..Theme::default() });
        assert_eq!(two.get(palette::series(1)), Some([4, 5, 6, 255]));
        assert_eq!(two.get(palette::series(2)), Some(DARK.series[2].rgba()));
        // A scale of fewer than two stops, which only code can build, takes the default.
        let short = resolve(&Theme { scale: Some(vec![c(0xffffff)]), ..Theme::default() });
        assert_eq!(short.get(SCALE_START), Some(DARK.scale[0].rgba()));
    }

    #[test]
    fn resolve_json_matches_resolve_and_names_bad_fields() {
        assert_eq!(resolve_json(EXAMPLE).unwrap(), resolve(&Theme::from_json(EXAMPLE).unwrap()));
        let err = resolve_json(r##"{"accent": "#12345g"}"##).unwrap_err();
        assert_eq!(err.field, "accent");
        assert!(err.to_string().contains("accent"), "{err}");
    }

    #[test]
    fn missing_tokens_fall_back_by_base() {
        assert_eq!(Theme::default().base(), Base::Dark);
        assert_eq!(Theme { paper: Some(true), ..Theme::default() }.base(), Base::Light);
        assert_eq!(Theme { paper: Some(false), ..Theme::default() }.base(), Base::Dark);
        assert_eq!(Theme { ground: Some(c(0xffffff)), ..Theme::default() }.base(), Base::Light);
        assert_eq!(Theme { ground: Some(c(0x101010)), ink: Some(c(0x000000)), ..Theme::default() }.base(), Base::Dark);
        assert_eq!(Theme { ink: Some(c(0x1e2125)), ..Theme::default() }.base(), Base::Light);
        assert_eq!(Theme { ink: Some(c(0xe8ebef)), ..Theme::default() }.base(), Base::Dark);
        // Luma 128 exactly is the dark side's edge: a ground of luma 128 is light.
        assert_eq!(Theme { ground: Some(c(0x808080)), ..Theme::default() }.base(), Base::Light);
        assert_eq!(Theme { ground: Some(c(0x7f7f7f)), ..Theme::default() }.base(), Base::Dark);
        let light = resolve(&Theme { paper: Some(true), accent: Some(c(0x0784ff)), ..Theme::default() });
        assert_eq!(light.get(palette::INK), Some(LIGHT.ink.rgba()));
        assert_eq!(light.get(palette::ACCENT), Some([0x07, 0x84, 0xff, 255]));
        assert_eq!(light.get(SCALE_START), Some(LIGHT.scale[0].rgba()));
    }

    const EXAMPLE: &str = include_str!("../../../spec/schemas/examples/theme.json");

    #[test]
    fn schema_examples_parse_and_invalid_ones_are_refused() {
        let theme = Theme::from_json(EXAMPLE).unwrap();
        assert_eq!(theme.ground, Some(c(0x131518)));
        assert_eq!(theme.series.as_ref().map(Vec::len), Some(8));
        assert_eq!(theme.scale.as_ref().map(Vec::len), Some(5));
        assert_eq!(theme.diverging.as_ref().map(Vec::len), Some(3));
        assert_eq!(theme.paper, Some(false));
        // serde's path gives the same theme.
        assert_eq!(serde_json::from_str::<Theme>(EXAMPLE).unwrap(), theme);

        let options: Value = serde_json::from_str(include_str!("../../../spec/schemas/examples/options.everything.json")).unwrap();
        let inner = Theme::from_value(&options["theme"]).unwrap();
        assert_eq!((inner.accent, inner.paper), (Some(c(0x0784ff)), Some(true)));

        let invalid = [
            (include_str!("../../../spec/schemas/examples/invalid/theme.nine-series.json"), "series"),
            (include_str!("../../../spec/schemas/examples/invalid/theme.short-hex.json"), "ink"),
            (include_str!("../../../spec/schemas/examples/invalid/theme.unknown-token.json"), "series-1"),
        ];
        for (json, field) in invalid {
            let err = Theme::from_json(json).unwrap_err();
            assert_eq!(err.field, field, "{err}");
            assert!(serde_json::from_str::<Theme>(json).is_err());
        }
        let bad_options: Value = serde_json::from_str(include_str!("../../../spec/schemas/examples/invalid/options.bad-theme.json")).unwrap();
        assert_eq!(Theme::from_value(&bad_options["theme"]).unwrap_err().field, "ink");
    }

    #[test]
    fn errors_name_the_field() {
        let cases = [
            ("[]", "theme"),
            ("{", "theme"),
            (r#"{"ground": 3}"#, "ground"),
            (r#"{"rule": "white"}"#, "rule"),
            (r##"{"muted": "#1234567"}"##, "muted"),
            (r##"{"up": "#gg0000"}"##, "up"),
            (r##"{"series": ["#000000", "#00000"]}"##, "series[1]"),
            (r#"{"series": []}"#, "series"),
            (r##"{"scale": ["#000000"]}"##, "scale"),
            (r##"{"diverging": ["#000000", "#ffffff"]}"##, "diverging"),
            (r#"{"diverging": "blue"}"#, "diverging"),
            (r#"{"paper": "yes"}"#, "paper"),
        ];
        for (json, field) in cases {
            let err = Theme::from_json(json).unwrap_err();
            assert_eq!(err.field, field, "{json}: {err}");
        }
        assert_eq!(Theme::from_json(r##"{"ink": "#ABCDEF"}"##).unwrap().ink, Some(c(0xabcdef)));
        assert_eq!(Theme::from_json("{}").unwrap(), Theme::default());
    }

    #[test]
    fn colour_parses_only_rrggbb() {
        assert_eq!(Color::parse("#629cfb"), Some(c(0x629cfb)));
        assert_eq!(c(0x629cfb).to_string(), "#629cfb");
        for bad in ["", "#", "629cfb", "#fff", "#629cfb0", "#+29cfb", "#62 cfb", "rgb(1,2,3)", "#é29cf"] {
            assert_eq!(Color::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn oklab_matches_ottosson() {
        // White is L = 1, a = b = 0; sRGB red is (0.62796, 0.22486, 0.12585)
        // (Ottosson 2020's matrices; the CSS Color 4 sample values agree).
        let white = from_srgb(c(0xffffff));
        assert!((white[0] - 1.0).abs() < 1e-4 && white[1].abs() < 1e-4 && white[2].abs() < 1e-4, "{white:?}");
        let red = from_srgb(c(0xff0000));
        assert!((red[0] - 0.62796).abs() < 1e-4 && (red[1] - 0.22486).abs() < 1e-4 && (red[2] - 0.12585).abs() < 1e-4, "{red:?}");
        // Every 8-bit grey and a spread of colours round-trip exactly.
        for v in 0..=255u32 {
            for rgb in [v * 0x010101, v << 16 | (255 - v) << 8 | 0x40, (v * 7 % 256) << 16 | v << 8 | (v * 13 % 256)] {
                assert_eq!(oklab::to_srgb(from_srgb(c(rgb))), c(rgb));
            }
        }
        // CIELAB: white is L* 100, mid grey #777777 L* 50.03 (CIE 15, D65).
        assert!((cielab(c(0xffffff))[0] - 100.0).abs() < 1e-3);
        assert!((cielab(c(0x777777))[0] - 50.03).abs() < 0.01, "{:?}", cielab(c(0x777777)));
        assert_eq!(delta_e76(c(0x123456), c(0x123456)), 0.0);
    }

    /// The largest ΔE76 between neighbouring steps of a resolved ramp.
    fn largest_step(palette: &Palette) -> f64 {
        let colour = |i: u8| palette.get(i).map(|[r, g, b, _]| Color { r, g, b }).unwrap();
        (SCALE_START..SCALE_END - 1).map(|i| delta_e76(colour(i), colour(i + 1))).fold(0.0, f64::max)
    }

    #[test]
    fn scale_ramp_steps_within_delta_e_2_5() {
        for (name, base) in [("dark", Theme::default()), ("light", Theme { paper: Some(true), ..Theme::default() })] {
            let step = largest_step(&resolve(&base));
            println!("default {name} scale: largest neighbouring step ΔE76 {step:.2}");
            assert!(step <= 2.5, "{name}: {step}");
        }
        // The ramp runs from the first stop to the last, through each stop.
        let ramp = scale_ramp(&CIVIDIS);
        assert_eq!((ramp[0], ramp[127]), (CIVIDIS[0], CIVIDIS[8]));
        assert_eq!(scale_ramp(&[c(0x336699)]), [c(0x336699); 128]);
        assert_eq!(scale_ramp(&[]), [c(0); 128]);
        // Two stops interpolate in Oklab: the middle of black to white is Oklab L 0.5, not sRGB 50%.
        let grey = scale_ramp(&[c(0x000000), c(0xffffff)]);
        let middle = (from_srgb(grey[63])[0] + from_srgb(grey[64])[0]) / 2.0;
        assert!((middle - 0.5).abs() < 0.01, "{middle}");
    }

    /// WCAG 2.2 contrast of every default token that has a bar, against its
    /// ground: series, up, down and accent 3:1, ink and muted 4.5:1 (REQ-THEME-040).
    #[test]
    fn default_tokens_meet_contrast_bars() {
        for (name, set) in [("dark", &DARK), ("light", &LIGHT)] {
            let mut graphic: Vec<(String, Color)> = vec![("up".into(), set.up), ("down".into(), set.down), ("accent".into(), set.accent)];
            graphic.extend(set.series.iter().enumerate().map(|(i, &s)| (format!("series-{}", i + 1), s)));
            for (token, colour) in graphic {
                let ratio = contrast(colour, set.ground);
                println!("{name} {token} {colour} on {}: {ratio:.2}", set.ground);
                assert!(ratio >= 3.0, "{name} {token}: {ratio}");
            }
            for (token, colour) in [("ink", set.ink), ("muted", set.muted)] {
                let ratio = contrast(colour, set.ground);
                println!("{name} {token} {colour} on {}: {ratio:.2}", set.ground);
                assert!(ratio >= 4.5, "{name} {token}: {ratio}");
            }
        }
        // WCAG's own extremes.
        assert!((contrast(c(0x000000), c(0xffffff)) - 21.0).abs() < 1e-9);
        assert_eq!(contrast(c(0x777777), c(0x777777)), 1.0);
    }

    /// Machado, Oliveira and Fernandes (2009), severity 1.0, on linear sRGB,
    /// as `discovery/bench/a11y-theming/palettes.py` applies them.
    const MACHADO: [(&str, [[f64; 3]; 3]); 3] = [
        ("deutan", [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.01182, 0.04294, 0.968881]]),
        ("protan", [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]]),
        ("tritan", [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.3039]]),
    ];

    /// CIELAB of `colour` as seen under `matrix` (`None`: normal vision).
    fn seen(colour: Color, matrix: Option<&[[f64; 3]; 3]>) -> [f64; 3] {
        let rgb = linear(colour);
        let rgb = match matrix {
            None => rgb,
            Some(m) => [0, 1, 2].map(|i| oklab::clip_unit(m[i][0] * rgb[0] + m[i][1] * rgb[1] + m[i][2] * rgb[2])),
        };
        cielab_from_linear(rgb)
    }

    /// CIEDE2000 (Sharma, Wu and Dalal 2005), kL = kC = kH = 1.
    fn ciede2000(p: [f64; 3], q: [f64; 3]) -> f64 {
        let deg = std::f64::consts::PI / 180.0;
        let c_bar = ((p[1] * p[1] + p[2] * p[2]).sqrt() + (q[1] * q[1] + q[2] * q[2]).sqrt()) / 2.0;
        let g = 0.5 * (1.0 - (c_bar.powi(7) / (c_bar.powi(7) + 25f64.powi(7))).sqrt());
        let prime = |lab: [f64; 3]| {
            let a = (1.0 + g) * lab[1];
            let c = (a * a + lab[2] * lab[2]).sqrt();
            let h = if a == 0.0 && lab[2] == 0.0 { 0.0 } else { lab[2].atan2(a).to_degrees().rem_euclid(360.0) };
            (lab[0], c, h)
        };
        let (l1, c1, h1) = prime(p);
        let (l2, c2, h2) = prime(q);
        let dl = l2 - l1;
        let dc = c2 - c1;
        let dh = if c1 * c2 == 0.0 {
            0.0
        } else if (h2 - h1).abs() <= 180.0 {
            h2 - h1
        } else if h2 - h1 > 180.0 {
            h2 - h1 - 360.0
        } else {
            h2 - h1 + 360.0
        };
        let dh_big = 2.0 * (c1 * c2).sqrt() * (dh / 2.0 * deg).sin();
        let l_bar = (l1 + l2) / 2.0;
        let c_bar_p = (c1 + c2) / 2.0;
        let h_bar = if c1 * c2 == 0.0 {
            h1 + h2
        } else if (h1 - h2).abs() <= 180.0 {
            (h1 + h2) / 2.0
        } else if h1 + h2 < 360.0 {
            (h1 + h2 + 360.0) / 2.0
        } else {
            (h1 + h2 - 360.0) / 2.0
        };
        let t = 1.0 - 0.17 * ((h_bar - 30.0) * deg).cos() + 0.24 * (2.0 * h_bar * deg).cos() + 0.32 * ((3.0 * h_bar + 6.0) * deg).cos()
            - 0.20 * ((4.0 * h_bar - 63.0) * deg).cos();
        let d_theta = 30.0 * (-((h_bar - 275.0) / 25.0).powi(2)).exp();
        let r_c = 2.0 * (c_bar_p.powi(7) / (c_bar_p.powi(7) + 25f64.powi(7))).sqrt();
        let s_l = 1.0 + 0.015 * (l_bar - 50.0).powi(2) / (20.0 + (l_bar - 50.0).powi(2)).sqrt();
        let s_c = 1.0 + 0.045 * c_bar_p;
        let s_h = 1.0 + 0.015 * c_bar_p * t;
        let r_t = -(2.0 * d_theta * deg).sin() * r_c;
        ((dl / s_l).powi(2) + (dc / s_c).powi(2) + (dh_big / s_h).powi(2) + r_t * (dc / s_c) * (dh_big / s_h)).sqrt()
    }

    #[test]
    fn ciede2000_matches_sharma_test_data() {
        // Pairs 1, 7 and 17 of Sharma, Wu and Dalal (2005), table 1.
        let cases = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
        ];
        for (p, q, expected) in cases {
            assert!((ciede2000(p, q) - expected).abs() < 1e-4, "{p:?} {q:?}: {}", ciede2000(p, q));
        }
    }

    /// The smallest CIEDE2000 between any two of `colours` under each vision.
    fn smallest_differences(colours: &[Color]) -> Vec<(&'static str, f64)> {
        let visions: Vec<(&str, Option<&[[f64; 3]; 3]>)> = std::iter::once(("normal", None)).chain(MACHADO.iter().map(|(n, m)| (*n, Some(m)))).collect();
        visions
            .into_iter()
            .map(|(name, matrix)| {
                let labs: Vec<[f64; 3]> = colours.iter().map(|&c| seen(c, matrix)).collect();
                let mut smallest = f64::INFINITY;
                for i in 0..labs.len() {
                    for j in i + 1..labs.len() {
                        smallest = smallest.min(ciede2000(labs[i], labs[j]));
                    }
                }
                (name, smallest)
            })
            .collect()
    }

    /// Eight categorical colours at least 15 CIEDE2000 apart under normal
    /// vision and the three simulated dichromacies (REQ-THEME-050), and up and
    /// down at least 50 apart (REQ-THEME-065).
    #[test]
    fn default_colours_stay_apart_under_cvd() {
        for (name, set) in [("dark", &DARK), ("light", &LIGHT)] {
            for (vision, smallest) in smallest_differences(&set.series) {
                println!("{name} series, {vision}: smallest ΔE00 {smallest:.1}");
                assert!(smallest >= 15.0, "{name} series {vision}: {smallest}");
            }
            for (vision, apart) in smallest_differences(&[set.up, set.down]) {
                println!("{name} up/down, {vision}: ΔE00 {apart:.1}");
                assert!(apart >= 50.0, "{name} up/down {vision}: {apart}");
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn from_json_never_panics(json in "\\PC{0,64}") {
            let _ = Theme::from_json(&json);
        }

        #[test]
        fn any_colour_token_resolves(rgb in 0u32..0x100_0000, token in 0usize..8) {
            let json = format!("{{\"{}\": \"{}\"}}", COLOR_TOKENS[token], c(rgb));
            let resolved = resolve_json(&json).unwrap();
            proptest::prop_assert_eq!(resolved.get(token as u8), Some(c(rgb).rgba()));
        }
    }

    /// REQ-THEME-070: a page-set series colour under 3:1 against the ground
    /// warns, naming the token and the ratio, and is still used.
    #[test]
    fn low_contrast_series_warn() {
        let theme = Theme::from_json(r##"{"ground":"#ffffff","series":["#000000","#eeeeee"]}"##).unwrap();
        let w = warnings(&theme);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].starts_with("series-2 #eeeeee is 1.16:1 against the ground #ffffff"), "{}", w[0]);
        assert_eq!(resolve(&theme).get(palette::series(1)), Some([0xee, 0xee, 0xee, 255]));
        assert!(warnings(&Theme::default()).is_empty(), "the defaults set no series");
    }
}
