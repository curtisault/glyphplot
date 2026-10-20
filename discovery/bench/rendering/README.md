# discovery/bench/rendering: throwaway

Measurements for `discovery/rendering.md`. Nothing here is part of glyphplot;
delete it once the report is settled. Each part is standalone. Run from the
repository root so mise gives the pinned toolchain.

## core-size: removed

`core-size/` measured what unicel's grid and sub-cell canvases cost in a wasm
module, one feature at a time (an empty module, a braille line, the canvases,
pixels, unicel as a git dependency, unicode-width). Its code copied unicel's
files and depended on unicel, so it was removed under ADR-013
(`spec/adr/013-independent-of-unicel.md`). Its numbers stand as recorded in
`discovery/rendering.md`; they cannot be rerun from this repository.

## font: what chart text adds to a subset font

`subset.py` subsets Iosevka Fixed Extended 34.9.0 to a base set of ranges and
then to the base plus each tier of chart-text ranges, and writes each subset
to `font/target/<tier>.woff2`. Its argument is the release zip
`PkgTTF-Unhinted-IosevkaFixed-34.9.0.zip`, downloaded from Iosevka's GitHub
releases (https://github.com/be5invis/Iosevka/releases/tag/v34.9.0).
`cmap.py` reports which chart-text characters a font has; give it any font
path.

    mise exec -- uvx --from 'fonttools[woff]==4.60.1' python -I \
      discovery/bench/rendering/font/subset.py <path>/PkgTTF-Unhinted-IosevkaFixed-34.9.0.zip
    mise exec -- uvx --from 'fonttools[woff]==4.60.1' python -I \
      discovery/bench/rendering/font/cmap.py <path to a .woff2 or .ttf>

## canvas: atlas draw cost by colour model, headless Chrome

`page.js` draws synthetic frames four ways (a glyph atlas keyed by
(code point, palette colour); the atlas with RGB keys; a tint by composite
with an atlas keyed by code point alone; `fillText` per cell) and full blocks
two ways (a rectangle a cell; one image of a pixel a cell). The atlas is an
offscreen canvas of one-cell slots in a near-square grid, cleared when full
(its comment gives the layout). `run.mjs` drives the installed Chrome through
playwright-core and prints JSON lines.

`run.mjs` and `backend.mjs` read the font from the path in `FONT` and stop
with a message when it is unset. Any monospace woff2 works, for example a
subset written by `font/subset.py` (`font/target/base.woff2`); the page
registers it as "bench mono". The recorded `results.jsonl` and
`backend.jsonl` were taken with unicel's font and an earlier atlas of 32 slots
across, each two cells wide; this atlas is the same mechanism in its own layout.

    cd discovery/bench/rendering/canvas && mise exec -- npm install
    export FONT=../font/target/base.woff2   # or any monospace .woff2
    mise exec -- node run.mjs 3 > results.jsonl
    mise exec -- node summarise.mjs results.jsonl
    mise exec -- node backend.mjs 3     # the WebGL renderer, and four cases with --disable-gpu

`page.js` also has `digits(fontPx, dpr)`, which `run.mjs` calls once.

## terminal: escape-code bytes and quantisation steps

    mise exec -- node discovery/bench/rendering/terminal/sgr.mjs
    mise exec -- node discovery/bench/rendering/terminal/steps.mjs
