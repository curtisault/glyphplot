# ADR-121: Extend the bundled font

Status: Accepted (2026-10-18), by the owner (open question 12 of the plan)

## Context

unicel's subset font has 1,215 code points: ASCII, `° · •`, arrows, box
drawing, blocks, geometric shapes, braille, octants and sextants. Chart text
needs more: U+2212 minus (Intl and d3-format write it), `µ ± × ÷ ‰ ′ ″`,
superscripts, `… – —`, the grouping spaces Intl writes (U+202F for fr-FR),
currency signs other than `$`, `≤ ≥ ≈ ≠ ∞`, a few Greek letters and accented
Latin. A missing glyph falls back to a system face whose advance may not be
0.6 em, so a `<pre>` row goes out of line (`discovery/rendering.md` §4.1,
cmap read with fontTools).

The options, measured by subsetting Iosevka Fixed Extended 34.9.0 as unicel's
`scripts/font.py` does (rendering.md §4.2, fonttools 4.60.1, woff2 bytes):

| Ranges | Glyphs | woff2 | Advances |
|---|---|---|---|
| unicel's, rebuilt | 1,215 | 18,128 B | 600 |
| + tier A: numbers, units, currency, marks | 1,361 | 25,344 B (+7,216) | 600 |
| + tier B: U+00C0–00FF, U+0100–017F | 1,551 | 28,180 B (+10,052) | 600 |
| + tier C: Greek, Cyrillic, U+2200–22FF | 2,183 | 47,320 B (+29,192) | 0 and 600 |

- Iosevka lacks 8 code points of tier A: two unassigned superscripts and six
  rare currencies.
- Tier C brings zero-width combining marks, which break the rule that every
  glyph is 0.6 em.
- The fourth option, no change with ASCII-only formatting (`-` for minus, `u`
  for micro), keeps the font at 18 KB and gives up typographic labels.

## Decision

- The bundled font is a subset of Iosevka Fixed Extended 34.9.0 (SIL OFL 1.1),
  family `glyphplot mono`, covering the base ranges (ASCII, `° · •`, arrows,
  box drawing, blocks, geometric shapes, braille, octants, legacy computing)
  plus tier A, every glyph 0.6 em, at most 32 KB, built only by `mise run
  build:font` with the repository's own script from Iosevka's upstream release
  (no file or script taken from unicel, [ADR-013](013-independent-of-unicel.md))
  (REQ-FONT-010, v0.2.0).
- Tier B follows (REQ-FONT-020, v0.5.0). Tier C is rejected.
- `@font-face` loads it from the package with `font-display: swap`, and every
  surface puts `"glyphplot mono"` first (REQ-FONT-030).

## Consequences

- Minus, units, ellipses and currencies draw in the grid's own face.
- A page pays 7.2 KB, then 10.1 KB, more font than unicel's.
- Greek beyond Δ Σ Ω µ σ, Cyrillic and CJK labels fall back to system faces;
  layout still counts their cells by `unicode-width`.
- Revisit tier C, without its marks, if Greek or Cyrillic labels are asked for.
