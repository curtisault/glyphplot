# web/fonts

- `glyphplot-mono.woff2`: the bundled font, family `glyphplot mono`, a subset
  of Iosevka Fixed Extended 34.9.0 Regular (REQ-FONT-010, ADR-121). Every
  glyph is 600 units wide (0.6 em). Generated and checked in.
- `unicode-range.txt`: the CSS `unicode-range` of the font's `@font-face`,
  built from the font's own cmap. Generated.
- `OFL.txt`: Iosevka's licence (SIL Open Font License 1.1), `LICENSE.md` at
  the release's tag `v34.9.0`. The font is distributed under it.

All three are written by `scripts/font.py` (`mise run build:font`) from
Iosevka's release zip, pinned by SHA-256; never edit them by hand.
`scripts/font.py --check` rebuilds the font and fails if any of them differs
byte for byte.
