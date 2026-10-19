# Rendering and colour

## Summary

- glyphplot should vendor unicel's grid, sub-cell canvases, palette ramp,
  blue-noise map and terminal glyph fallback (about 1,240 lines of Rust
  before tests) with a provenance header, not depend on unicel. No unicel
  crate is published (crates.io answers 404 for `unicel` and `unicel-core`
  on 2026-10-18), and crates.io refuses a crate with a git dependency, so a
  git dependency would stop glyphplot itself from publishing.
- The grid and the braille canvas cost 3,043 bytes gzipped over an empty
  wasm-bindgen module (8,901 against 5,858); all four canvases and the colour
  canvases 7,145; `Pixels` 6,014 more. Taking the same code as a git
  dependency gives 9,045 bytes: LTO drops unicel's `Meta` and serde.
- Colour should stay a `u8` index per cell, but into a per-chart palette of
  up to 255 entries (theme roles, series colours and a scale ramp quantised
  to at least 128 steps) instead of unicel's 64. At 128 steps the largest
  step of viridis is ΔE76 2.26, under the 2.3 that people can just see; at
  64 it is 4.55. The cell stays 6 bytes; RGB ink and back would make it 12.
- The colour model is not what makes the atlas slow. unicel's atlas is keyed
  by (code point, colour). A braille scatter coloured by value has 944
  distinct pairs a frame at 80 × 24 even with 64 colours, so the 1,024-slot
  atlas hits 3.6 % of the time and a frame takes 60 ms. At 320 × 120 it
  takes 1.1 to 1.4 s. Two ways out cost the same for a palette and for
  RGB: an atlas keyed by code point alone, tinted by compositing (2.4–2.8 ms and
  33–35 ms), or `fillText` per cell with no atlas (0.9 ms and 16 ms). In
  headless Chrome, on the GPU and off it, `fillText` was the fastest way
  to draw in every case.
- The bundled font lacks most of what chart text needs: U+2212 minus, `µ`,
  `±`, `×`, `…`, `–`, every currency sign but `$`, `‰`, superscripts,
  `≤ ≥ ≈`, Greek and accented Latin letters. Adding them costs 7,216 bytes
  of woff2 for numbers and units, or 10,052 bytes with Western European
  letters.
- Cells come from the element: the font size sets the cell (0.6 em wide,
  twice as tall, so braille dots are square), and the box sets `cols × rows`.
  In headless Chrome at 1× pixels, the closest pair of digits (0 and 3)
  differs by 1 pixel at an 8 px font and by 7 pixels from 11 px up. This is
  a proxy for legibility; the owner judges it by eye.

## Scope and method

What I read: the brief (`README.md`, "The briefs", topic 4); unicel's `spec/SPEC.md` §3, §6, §7 and §8;
ADRs 003, 004, 014, 016, 026, 027, 118, 120 and 121;
`crates/unicel/src/{grid,subcell,palette,pixels,meta,util,bluenoise,glyphs,mono,lib}.rs`;
`crates/unicel/Cargo.toml`, `crates/wasm/{Cargo.toml,src/lib.rs}`,
`crates/cli/src/paint.rs`; `web/src/{mount,element,frame,blocks,fit}.ts`;
`web/fonts/`; `scripts/font.py`. unicel was read at local HEAD `282c3c8`.

What I measured, all under `discovery/bench/rendering/` (its README says
how to rerun each part):

- `core-size/`: a crate that copied unicel's files unchanged, since removed
  under ADR-013 (its numbers stand; they can't be rerun from this
  repository). It was built with wasm-pack 0.15.0 `--target web --release`, using unicel's release
  profile and wasm-opt flags. It is built one feature at a time. Sizes are
  `gzip -9 -c file | wc -c`.
- `font/subset.py`: Iosevka subset with unicel's options and more ranges, via
  `uvx --from 'fonttools[woff]==4.60.1'`.
- `canvas/`: synthetic frames drawn in headless Chrome 155.0.8059.82 via
  playwright-core 1.59.1, using the installed Chrome. Each case is 10 warm-up
  frames and then 60 timed frames. Each frame is a full redraw followed by
  `getImageData(0, 0, 1, 1)`, so the raster has finished when the clock
  stops. Each case ran 3 times. I report the min and median of the per-run
  medians.
- `terminal/`: escape-code bytes for one full frame, written the way
  unicel's CLI writes them, and the quantisation steps in CIE Lab. These are
  deterministic arithmetic, so they ran once.

Machine: Apple M5 Pro, 18 cores, 48 GB, macOS, 2026-10-18. Other discovery
agents were benchmarking on the machine at the same time. `uptime` load
averages went from 3.18 at the start to 7.0–9.2 during the canvas runs
(7.6 at the end).
Timings are therefore upper bounds, and their ratios matter more than their
values.

Not covered: Safari and headed (GPU-composited) Chrome. Rendering with a
real GPU may order the renderers differently (see Open questions). Also not
covered: Firefox, the SVG and PNG exports, and visual judgement of digits,
which is the owner's.

## 1. What glyphplot reuses from unicel

| unicel file | Lines (code before tests / total) | glyphplot | Why |
|---|---|---|---|
| `crates/unicel/src/grid.rs` | 275 / 386 | vendor | The frame buffer the owner chose (`u32` code point, `u8` ink, `u8` back, `WIDE_TAIL`, `NO_BACK`), its clipping draws, `text()`. |
| `subcell.rs` | 687 / 875 | vendor | `Dots`, `Sextants`, `Quadrants` and `Octants`, plus the colour canvases. Braille lines and scatter, octant areas and bars. 256 lines are the octant table. |
| `palette.rs` | 52 / 74 | vendor | `Ramp` maps a value to a run of palette indices, which is the scale ramp of §2. |
| `bluenoise.rs` | 51 / 59 | vendor | The 32 × 32 threshold map for one-ink density (a heatmap in dots). |
| `util.rs` | 166 / 241 | vendor `braille`, `BAYER`, `dither` (about 30 lines) | `Rng`, `Mulberry32` and `Replay` serve seeded art. A chart's data is given. |
| `glyphs.rs` | 146 / 196 | vendor | Converts octants to braille and others for terminal fonts (ADR-120). The CLI and the text export need it. |
| `mono.rs` | 124 / 159 | adapt | The one-ink checks (ink share, rows that differ, glyphs the font has). They suit a chart contract; they are tied to unicel's `registry::Entry` and `Piece`. |
| `pixels.rs` | 220 / 250 | do not vendor | It is for pictures. In one ink it stretches each frame between its 2nd and 98th brightness percentiles (REQ-PIX-500), which would misstate data values. In colour it dithers by up to ±0.035 a channel (`SPREAD = 0.07`), which moves a cell off its exact scale step. A heatmap needs value to step, unstretched. |
| `meta.rs` | 217 / 217 | drop | A piece's `cols`, `rows`, `fps` and `category` do not apply: a chart's size comes from its element. The `Charset` idea carries over. |
| `web/src/blocks.ts` | 55 | vendor | Exact shapes for octants, sextants and wedges (ADR-027). |
| `web/src/frame.ts` (`paint`) | 85 | adapt | A stateless draw of one frame, for exports. |
| `web/src/mount.ts` | 499 | rewrite, keeping its rules | Views re-made after every frame, the font load that clears the atlas, a ResizeObserver that ignores a width of 0, the IntersectionObserver and visibility gates. The (code point, colour) atlas does not carry over (§3). |
| `web/src/fit.ts` | 35 | drop | It fits a fixed `cols × rows` into a box. A chart is sized the other way round (§5). |
| `web/fonts/unicel-mono.woff2`, `scripts/font.py` | 17,984 B; 129 lines | vendor and extend | Same face. Add chart-text ranges (§4). |

That makes about 1,240 lines of Rust and 55 of TypeScript taken as they are,
and about 210 lines adapted.

### 1.1 Depend, take as a git dependency, or vendor

**Published crate.** None exists. `curl https://crates.io/api/v1/crates/unicel`
gave 404 `crate 'unicel' does not exist` on 2026-10-18, and so did
`unicel-core`. unicel's `SPEC.md` §1.3 says "no crates.io release" and calls
the name a placeholder. `crates/unicel/Cargo.toml` carries no
`publish = false` (the wasm and CLI crates do). Even so, it is one crate
holding the core and 207 pieces behind 17 features, with
`serde`, `serde_json` and `unicode-width` as dependencies. To publish a
`unicel-core`, unicel would need to:

1. Split `grid`, `subcell`, `palette`, `bluenoise`, `glyphs` and the braille
   and dither helpers into a crate with no serde and no pieces.
2. Make `unicel` depend on it.
3. Settle the name (the placeholder in its §1.3).
4. Release it, and keep semver for both projects.

Every change glyphplot needs in the core, such as REQ-COLOR-010 below,
would then have to land in unicel and be released first.

**Git dependency.** `unicel = { git = "https://github.com/curtisault/animate-unicode", rev = "53af162…", default-features = false }`
builds and works (the `git` feature of `core-size`, since removed). I found four costs:

- crates.io does not allow a published crate to depend on a git repository
  ("crates.io does not allow packages to be published with dependencies on
  code published outside of crates.io",
  https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html).
  The owner has reserved `glyphplot` on crates.io, so a git dependency
  blocks that release until unicel publishes.
- Only pushed commits can be pinned. unicel's local HEAD `282c3c8` is not on
  the remote (`git ls-remote` gives `53af162` for HEAD), so the bench pins
  `53af162`.
- On this machine, cargo's built-in git fetch failed with "could not read
  from remote repository" (3 retries). It worked with
  `CARGO_NET_GIT_FETCH_WITH_CLI=true`. glyphplot would need that in
  `.cargo/config.toml`, and network access for a cold signoff.
- It compiles serde, serde_derive, serde_json and syn for unicel's `Meta`.
  A cold build took a median of 3.68 s against 3.48 s vendored (3 runs
  each). The wasm came out 144 bytes larger (9,045 against 8,901 gzipped).

**Vendoring.** Copy the files listed above into the glyphplot core with a
header naming unicel, its licence (MIT, the same owner) and the source
revision. The cost is drift: a fix in one copy does not reach the other.
A check script can keep that small (REQ-CORE-020 below).

## 2. Colour

unicel holds a cell to a `u8` ink and a `u8` back, indexing a palette of 2
to 64 colours (`crates/unicel/tests/contract.rs:148`, ADR-003); 255 is
`NO_BACK`. The buffer can already address 255 colours. The 64 is a contract
rule, not a property of the buffer.

Options:

- **(a)** unicel as it is: a `u8` index into at most 64 colours.
- **(b)** a `u8` index into a per-chart palette of up to 255 entries (0 to
  254), built from the theme and quantised from the chart's scales.
- **(c)** RGB a cell: `u32` (or 3 × `u8`) for ink and for back.

### 2.1 Memory

This is arithmetic from the buffer layouts, with cells = cols × rows.

| Model | Bytes a cell | 80 × 24 (1,920 cells) | 320 × 120 (38,400 cells) | Plus |
|---|---|---|---|---|
| (a) | 4 + 1 + 1 = 6 | 11,520 | 230,400 | 64 × 4 B palette |
| (b) | 6 | 11,520 | 230,400 | ≤ 255 × 4 = 1,020 B palette |
| (c) `u32` ink and back | 4 + 4 + 4 = 12 | 23,040 | 460,800 | — |
| (c) 3 × `u8` ink and back, plus a flag | 4 + 3 + 3 + 1 = 11 | 21,120 | 422,400 | — |

The shell keeps a copy of the last frame to redraw only changed cells (as
`mount.ts` does), which doubles each figure on the JS side. A `ColorCanvas`
holds a `u8` colour and an `f32` depth per dot, 8 dots a cell, so 41 bytes a
cell or 1.57 MB at 320 × 120. With RGB per dot that becomes 65 bytes a
cell. One `Float64Array` column of a million points is 8 MB. Memory does not
decide the colour model.

### 2.2 Quantising a continuous scale

`terminal/steps.mjs` measures the largest step between neighbouring palette
entries in CIE Lab (ΔE76; about 2.3 is the smallest step people see):

| Steps | 5-stop ramp of the bench | viridis, 9 stops |
|---|---|---|
| 16 | 17.54 | 18.52 |
| 32 | 8.53 | 9.21 |
| 64 | 4.20 | 4.55 |
| 128 | 2.08 | 2.26 |
| 200 | 1.33 | 1.44 |
| 255 | 1.04 | 1.13 |

A 64-colour palette bands a smooth gradient visibly, at about twice the step
that can be seen. From 128 steps a quantised scale shows no visible step.
That leaves (b) 126 entries for theme roles and series colours.

### 2.3 Drawing cost on a canvas

Workloads (`canvas/page.js`), each a full redraw of a field that changes
every frame:

- **density**: a braille heatmap. Each cell lights 0–8 dots by its value and
  takes the colour of its value. Glyph and colour move together.
- **scatter**: a braille scatter coloured by value. Each cell has an
  arbitrary dot mask and a colour from its value; a third of the cells are
  empty. Glyph and colour are independent.
- **blocks**: a full-block heatmap, `█` a cell.

Renderers:

- **atlas**: as measured, unicel's layout: 32 × 32 slots of two cell
  widths, keyed by (code point, colour), cleared when full. The bench now
  packs slots its own way (its README). `/4096` means 4,096 slots.
- **tint**: an atlas of white glyphs keyed by code point alone, drawn into a
  mask layer. A colour layer holds one pixel a cell, put with
  `putImageData` and scaled up without smoothing. The two are composited
  with `destination-in`, then the ground goes under with `destination-over`.
- **direct**: `fillStyle` and `fillText` per cell, no atlas.
- **rects**: one `fillRect` a cell. This is unicel's path for `█`.
- **image**: one image of a pixel a cell, scaled. This is unicel's
  `drawPixels` path.

The cell is 10 device px wide at 80 × 24 and 8 px at 320 × 120. Times are
ms a frame: the min and median of the 3 runs' medians, then the median p95.

| Size | Workload | Colours | Renderer | Distinct (glyph, colour) a frame | Atlas hits | Frame ms min / median (p95) |
|---|---|---|---|---|---|---|
| 80 × 24 | — | — | fill and readback only | — | — | 0.0 / 0.0 (0.1) |
| 80 × 24 | density | 64 | atlas | 24 | 100 % | 3.2 / 3.2 (3.5) |
| 80 × 24 | density | 255 | atlas | 87 | 100 % | 3.1 / 3.2 (3.5) |
| 80 × 24 | density | RGB | atlas | 195 | 100 % | 2.9 / 3.3 (3.8) |
| 80 × 24 | density | 64 / 255 / RGB | tint | 24 / 87 / 195 | 100 % | 3.0–3.2 / 3.2–3.3 |
| 80 × 24 | density | RGB | direct | 195 | — | 1.0 / 1.1 (1.3) |
| 80 × 24 | scatter | 64 | atlas | 944 | 3.6 % | 54.9 / 60.1 (66.5) |
| 80 × 24 | scatter | 255 | atlas | 1,176 | 0.9 % | 56.6 / 60.7 (68.1) |
| 80 × 24 | scatter | RGB | atlas | 1,209 | 0.6 % | 56.4 / 58.2 (63.2) |
| 80 × 24 | scatter | 255 / RGB | atlas/4096 | 1,176 / 1,209 | 9.1 / 5.3 % | 52.1 / 53.9; 53.5 / 55.2 |
| 80 × 24 | scatter | 64 / 255 / RGB | tint | 944–1,209 | 100 % | 2.1–2.2 / 2.4–2.8 (3.0–3.1) |
| 80 × 24 | scatter | RGB | direct | 1,209 | — | 0.8 / 0.9 (1.1) |
| 80 × 24 | blocks | 255 / RGB | rects | 85 / 194 | — | 0.5 / 0.5 |
| 80 × 24 | blocks | 255 / RGB | image | 85 / 194 | — | 0.9 / 0.9 |
| 320 × 120 | — | — | fill and readback only | — | — | 0.3 / 0.3 (0.5) |
| 320 × 120 | density | 64 / 255 / RGB | atlas | 24 / 88 / 212 | 100 % | 44.9–45.8 / 46.1–48.0 |
| 320 × 120 | density | 64 / 255 / RGB | tint | 24 / 88 / 212 | 100 % | 45.1–45.6 / 45.9–46.6 |
| 320 × 120 | density | RGB | direct | 212 | — | 20.3 / 20.7 (22.6) |
| 320 × 120 | scatter | 64 | atlas | 5,231 | 3.6 % | 1,126 / 1,130 (1,269) |
| 320 × 120 | scatter | 255 | atlas | 14,174 | 0.9 % | 1,161 / 1,337 (1,358) |
| 320 × 120 | scatter | RGB | atlas | 17,440 | 0.6 % | 1,173 / 1,199 (1,299) |
| 320 × 120 | scatter | 255 / RGB | atlas/4096 | 14,174 / 17,440 | 3.6 / 2.4 % | 1,194 / 1,207; 1,201 / 1,367 |
| 320 × 120 | scatter | 64 / 255 / RGB | tint | 5,231–17,440 | 100 % | 32.8–33.2 / 32.8–34.5 |
| 320 × 120 | scatter | RGB | direct | 17,440 | — | 15.5 / 16.1 (17.4) |
| 320 × 120 | blocks | 255 / RGB | rects | 85 / 211 | — | 9.1 / 9.2; 9.6 / 9.7 |
| 320 × 120 | blocks | 255 / RGB | image | 85 / 211 | — | 5.8 / 6.3; 6.0 / 6.3 |

Which backend: headless Chrome drew with the GPU (WebGL renderer
"ANGLE Metal Renderer: Apple M5 Pro"). `canvas/backend.mjs` repeated
four cases with `--disable-gpu` (SwiftShader), 3 runs each, at load 4.7 to 7.1.
At 320 × 120 the density atlas went from a median of 45.9 to 34.1 ms,
`fillText` from 20.1 to 20.6 ms and the scatter tint from 33.6 to 34.7 ms.
At 80 × 24 the atlas went from 3.1 to 1.8 ms and `fillText` stayed at 1.0 ms.
`fillText` was fastest with the GPU on and with it off.

What the table shows:

- When glyph and colour move together (density), every model's pairs fit in
  1,024 slots and the colour model does not change the frame time.
- When they are independent (a scatter or a dot plot coloured by a third
  variable), the (code point, colour) atlas thrashes even with unicel's 64
  colours: 256 braille glyphs × 64 colours is 16,384 keys. Each miss
  rasterises into the atlas and then draws from a canvas that has just
  changed. A 4,096-slot atlas does not save it (53.9–55.2 ms). The colour model cannot
  repair this; only an atlas with a different key can.
- Keying the atlas by code point and tinting by composite makes the hit rate
  100 % and the cost independent of the colour model.
- In headless Chrome, `fillText` per cell beat every atlas, even at a 100 %
  hit rate: 20.7 against 46.1 ms for density at 320 × 120, and with the GPU off as well (20.6 against 34.1 ms). unicel never
  measured its atlas against `fillText`: its plan cites ascii.rest as proof
  (`docs/IMPLEMENTATION_PLAN.md:62`, `:582`). Headless Chrome drew on the GPU (Metal), but a headed
  window composites differently and Safari is another engine, so the choice of renderer
  needs the owner's browsers (Open questions 3).
- A full redraw of 320 × 120 braille costs 16–21 ms at best, so a streaming
  full-screen heatmap holds 30 frames a second, not 60. Pointer moves change
  a few cells, and those cells must be the only ones redrawn (REQ-RENDER-030).
- For block heatmaps, one image of a pixel a cell beats a rectangle a cell
  from about 320 × 120 (6.3 against 9.2 ms) and loses at 80 × 24 (0.9
  against 0.5 ms). Neither depends on the colour model.

### 2.4 Terminal

unicel's CLI writes truecolour only (`\x1b[38;2;r;g;bm`,
`crates/cli/src/paint.rs:167`), skips a code when the colour has not
changed, and redraws only changed cells. `terminal/sgr.mjs` counts the bytes
of one full frame written that way:

| Frame | Size | Truecolour, RGB | Truecolour, 255 steps | Truecolour, 64 steps | 256-colour (`38;5;n`) |
|---|---|---|---|---|---|
| braille density heatmap | 80 × 24 | 39,175 B (1,871 codes) | 38,348 | 33,259 | 10,438 (429) |
| block heatmap (back colours) | 80 × 24 | 35,335 | 34,508 | 29,419 | 6,598 |
| three-series line chart with an axis | 80 × 24 | 3,282 (53) | 3,282 | 3,282 | 3,068 |
| braille density heatmap | 320 × 120 | 777,760 (37,370) | 765,453 | 668,290 | 204,601 (8,545) |
| block heatmap | 320 × 120 | 700,960 | 688,653 | 591,490 | 127,801 |
| line chart | 320 × 120 | 47,204 (395) | 47,204 | 47,204 | 45,162 |

- Truecolour bytes barely depend on whether cells are RGB or 255 palette
  steps (2 % apart), because neighbouring cells of a smooth field rarely
  share an exact colour either way.
- 256-colour output is 3.7 to 5.5 times smaller for heatmaps, but the
  xterm cube and greys map the whole viridis scale onto 15 distinct colours,
  with a mean error of 37.4 and a maximum of 51.7 RGB units over 1,001
  samples. A heatmap in 256 colours is a 15-step heatmap.
- Line charts are the same size in every mode: few cells carry ink.
- A full-screen heatmap at 30 frames a second in truecolour is about
  23 MB/s. Only the cells that changed may be written (unicel already does
  this), and a streaming heatmap in a terminal needs a lower rate.
- With (b), the CLI maps each palette entry to its nearest 256-colour once
  a palette. With (c) it maps every cell, every frame.

### 2.5 One ink and paper

- **One ink** (a `<pre>`, a terminal with no colour): colour cannot carry a
  value in any model. A one-ink heatmap shows value as dot density (the
  density workload: 0–8 braille dots, or octants against the blue-noise
  map). Series are told apart by glyph (braille dots, solid octants, dashed
  box drawing) and by direct labels. This is authored per chart, as
  unicel's `Mono::Authored` is, and it does not depend on the colour model.
- **Paper and themes**: with (a) or (b), an index names a role (ground, axis,
  label, series 3, scale step 97), and the shell turns indices into colours.
  A theme change, dark to light, or paper swaps the palette table and
  repaints with no new wasm frame. A high-contrast or colour-blind theme is
  a table too. With (c), the chart has already resolved colours, so every
  theme change is a new frame. The shell cannot tell a series colour from a
  scale colour, so it cannot re-theme or downgrade after the fact.

### 2.6 What decides it

The atlas results take away (c)'s worst cost, because a code-point-keyed
renderer is model-blind. What decides between (b) and (c) is everything
else. (b) keeps the owner's buffer format unchanged at 6 bytes a cell. It
keeps themes, paper and the terminal's 256-colour mapping as table lookups
in the shell, and 128 or more scale steps fall below what the eye
separates. (c) gains exact colours, plus scales and themes with no
255-entry budget, and pays with a frame per theme change and per-cell
quantisation in the terminal. I recommend (b). (c) stays open for a chart
that must show more than about two continuous scales at once.

## 3. The renderer

The (code point, colour) atlas does not carry over. The measurements leave
two candidates, which the owner's browsers have to decide between:

- **direct `fillText`**. It was fastest in headless Chrome and needs no
  atlas, so it has no key space. Each glyph is drawn from the font at its
  cell, clipped to it only when a slot is made. A glyph wider than its cell
  (a fallback face) needs a clip per cell.
- **tint by composite**, with the atlas keyed by code point. It was 100 %
  hits for every model and 2 to 3 times slower than `fillText` here. It
  clips each glyph to its slot as unicel does, so fallback glyphs cannot
  spill.

Either way, block shapes stay exact fills (`blocks.ts`, ADR-027).
Back-coloured half-block heatmaps keep unicel's one-image path. Only
changed cells are redrawn.

## 4. Text in charts

### 4.1 What the bundled font has

`fontTools` cmap of `web/fonts/unicel-mono.woff2`, 1,215 code points
(`font/cmap.py`, run with `uvx --from 'fonttools[woff]==4.60.1' python -I`): U+0020–007E,
U+00B0, U+00B7, U+2022, U+2190–21FF, U+2500–25FF, U+2800–28FF,
U+1CD00–1CDE5, U+1CEA0–1CEAF, and most of U+1FB00–1FBFF.

| Needed for | Code points | In the font |
|---|---|---|
| Digits, A–Z, a–z, `$ % . , + - ( )` | ASCII | yes |
| Degree, middle dot, bullet | `°` `·` `•` | yes |
| Arrows, triangles, `● ■ □ ◆` | U+2190–21FF, U+25A0–25FF | yes |
| Minus sign (Intl and d3-format write it) | U+2212 `−` | **no** |
| Units | `µ` U+00B5, `±`, `×`, `÷`, `‰`, `′ ″`, `²` `³`, U+2070–209F | **no** |
| Truncation and ranges | `…` U+2026, `–` U+2013, `—` U+2014 | **no** |
| Grouping spaces (Intl writes U+202F for fr-FR) | U+00A0, U+2007, U+2009, U+202F | **no** |
| Currency | `€ £ ¥ ¢ ₹ ₿ ₩` | **no** (only `$`) |
| Maths in labels | `≤ ≥ ≈ ≠ ∞`, `Δ Σ σ Ω` | **no** |
| Accented labels | U+00C0–00FF, U+0100–017F | **no** |
| Marks | `✓ ✗ ★`, U+FFFD | **no** |

A missing glyph falls through to a system face (REQ-FONT-020 in unicel),
whose advance may not be 0.6 em. In a `<pre>` that row goes out of line. On
a canvas the glyph is drawn in its cell but in another face.

### 4.2 What adding them costs

`font/subset.py` subsets Iosevka Fixed Extended 34.9.0 as unicel's
`scripts/font.py` does (no hinting, no layout features, woff2):

| Ranges | Glyphs | woff2 bytes | Widths (units of 1000) |
|---|---|---|---|
| unicel's, rebuilt | 1,215 | 18,128 (checked in: 17,984; the name table differs) | 600 |
| + tier A: numbers and units: U+00A0–00BF, `× ÷`, U+2007–200A, U+2010–2015, U+2018–201F, `…`, U+202F, `‰`, `′ ″`, U+2070–209F, U+20A0–20C0, `№ ™`, `−`, `≈ ≠ ≤ ≥ ∞`, `Δ Σ Ω µ σ`, `✓ ✗ ★ ☆`, U+FFFD | 1,361 | 25,344 (+7,216) | 600 |
| + tier B: U+00C0–00FF, U+0100–017F | 1,551 | 28,180 (+10,052) | 600 |
| + tier C: Greek, Cyrillic, U+2200–22FF | 2,183 | 47,320 (+29,192) | **0 and 600** |

- Iosevka lacks only 8 code points of tier A: U+2072 and U+2073
  (unassigned) and six rare currencies (U+20A0, 20A2, 20B0, 20B6, 20B7,
  20BB).
- Tier C brings zero-width combining marks, which break unicel's
  "every glyph 600" check. It would need those marks left out.
- Every space in this face is 600 units, the thin, figure and narrow
  no-break spaces included (cmap and `hmtx` read with fontTools). In a grid
  there are no part-cell spaces. A formatter's U+2009 or U+202F becomes one
  whole cell, and a layout that counts characters counts cells.

### 4.3 Digits at small sizes

`window.digits` in `canvas/page.js` draws each digit as the atlas does
(centred, `textBaseline = "middle"`, in unicel's font when measured; the
bench now takes any font from `FONT`) into one cell. It
thresholds coverage at 50 %, then reports the closest pair of digits and how
many pixels tell them apart (one run, headless Chrome):

| Font px | Pixel ratio | Cell px | `8` tall px | Closest pair | Pixels apart |
|---|---|---|---|---|---|
| 8 | 1 | 5 × 10 | 6 | 0 / 3 | 1 |
| 9 | 1 | 5 × 11 | 7 | 3 / 8 | 4 |
| 10 | 1 | 6 × 12 | 7 | 0 / 8 | 4 |
| 11 | 1 | 7 × 13 | 8 | 0 / 3 | 7 |
| 12 | 1 | 7 × 14 | 9 | 3 / 8 | 7 |
| 8 | 2 | 10 × 19 | 12 | 3 / 8 | 12 |
| 10 | 2 | 12 × 24 | 15 | 0 / 3 | 23 |
| 12 | 2 | 14 × 29 | 18 | 0 / 3 | 29 |

At 1× an 8 px font leaves 0 and 3 one pixel apart. From 11 px the
separation doubles. At 2× every size from 8 px separates by 12 pixels or
more. This is a proxy for legibility, not legibility itself. The owner
checks it by eye (Open questions 5).

### 4.4 Layout

A chart is laid out in whole cells, from its `cols × rows`, before anything
is drawn:

- **Gutters.** The y axis takes the widest tick label, plus one cell for the
  tick and one for the axis line. The x axis takes one row for the line and
  one for labels. A title, if any, takes one row, and an outside legend one
  row.
- **Plot area.** What is left. Data coordinates map to dots in it: 2 × 4 a
  cell for braille and octants.
- **Ticks.** The tick generator (agent 5) is asked for decreasing counts
  until the labels fit with one cell of gap: label width + 1 ≤ cells
  between ticks. A label is centred on its tick's cell and clamped inside
  the plot's span. Any that would still overlap are dropped, left to right.
  This is integer arithmetic, so the same layout comes out on every
  surface.
- **Z-order.** One glyph per cell makes draw order the z-order: data, then
  grid lines on empty cells only, axes, labels, annotations, the
  crosshair, and the readout last. A label clears the cells under it to the
  ground, so text never shares a cell with data.
- **Readouts and tooltips.** A box-drawing frame placed in the quadrant
  away from the pointer and clipped to the chart.
- **Legends.** `■ name` runs, truncated with `…` (tier A) when they do not
  fit.
- **Text width.** Labels from the page can hold any Unicode. Measuring them
  needs East Asian Width in release builds, not only in debug as in unicel.
  `unicode-width` 0.2.2's `UnicodeWidthStr::width` added 4,882 bytes
  gzipped (13,783 against 8,901, the `width` feature). Wide glyphs use
  `WIDE_TAIL`, which every chart has to handle, since labels come from users.

## 5. Resolution

unicel inverts what glyphplot needs. A piece's `META` fixes `cols × rows`,
and the canvas cell is the element width over `cols`
(`mount.ts` `size()`, REQ-MOUNT-050, `fitBox`). A chart instead takes its
cell from the font size and its `cols × rows` from its box:

- **Cell.** `cw = round(0.6 × fontPx × dpr)` device pixels and `ch = 2 × cw`.
  The font size is the element's computed `font-size`. At exactly 1:2,
  braille and octant dots are square (cw/2 × ch/4), so a scatter keeps its
  aspect, and whole-pixel cells keep axis ticks from jittering. unicel's
  rounded per-cell edges (`xs[]`) make cells 1 px uneven.
- **Grid.** `cols = floor(boxW × dpr / cw)`, `rows = floor(boxH × dpr / ch)`,
  from the content box. The remainder is split as padding.
  `devicePixelContentBoxSize` gives exact device pixels where it exists.
  MDN's compatibility data lists it as Chrome 84 and Firefox 108, and
  `version_added: false` for Safari
  (https://github.com/mdn/browser-compat-data/blob/main/api/ResizeObserverEntry.json),
  so Safari needs `contentBoxSize × devicePixelRatio`.
- **Height.** A block element has no height of its own, so a chart needs a
  CSS height, a `rows` attribute, or a default. This is agent 8's to settle
  with the element.
- **Resize.** A ResizeObserver, coalesced to one animation frame, works out
  `cols × rows`. If they changed, the shell calls the chart's resize, which
  reallocates the grid (one `Grid::new`), lays out again and draws a frame.
  The view state is kept in data coordinates, so the same domain stays in
  view. Memory views are made again (the buffer may move), and glyph caches
  are cleared only when `cw` changes. A width of 0 (hidden) is ignored, as
  REQ-MOUNT-110 does.
- **Minimum useful size.** These are proposals, worked out from the gutters
  of §4.4 with 5-cell labels. They are design arithmetic, **(unverified)**
  by eye:

| Chart | Plot area | With axes | Below it |
|---|---|---|---|
| sparkline | 8 × 1 | (no axes) | draws what fits, down to 1 × 1 |
| line, area, scatter | 16 × 4 (32 × 16 dots) | 23 × 6 | drops axes, then becomes a sparkline |
| candlestick | 20 candles: 20 × 6 (one cell each; 10 cells with left and right half blocks) | 28 × 8 | fewer candles, then no axes |
| bar, histogram | one cell a bar plus a one-cell gap: 2n − 1 × 4 | +7 × +2 | thinner bars (octant halves), then no labels |
| heatmap | one cell a bin, or two bins a cell with `▀` and a back colour | +7 × +2 | bins merged |

A chart MUST draw at every size from 1 × 1 up without panicking
(REQ-LAYOUT-030). Below its minimum it sheds parts in a fixed order.

## Measurements

All on an Apple M5 Pro, 18 cores, 48 GB, macOS, 2026-10-18, shared with
other benchmarking agents (load averages 3.2 to 9.2). Sizes and byte counts
are exact and were measured once. Timings are 3 runs, min and median of
the per-run medians.

| What | Number | How |
|---|---|---|
| Empty wasm-bindgen module | 5,858 B gzip (13,050 raw) | `core-size --features empty`, wasm-pack 0.15.0 `--release`, `gzip -9` |
| + `Grid` + `Dots`, one braille line | 8,901 B (+3,043) | `--features braille` |
| + all four canvases and colour canvases, `Ramp` | 13,003 B (+7,145 over empty) | `--features canvases` |
| + `Pixels` and blue noise | 19,017 B (+6,014) | `--features pixels` |
| The same as `braille`, unicel as a git dependency | 9,045 B (+144 over vendored) | `--features git`, rev `53af162` |
| `unicode-width` str width in release | +4,882 B (13,783) | `--features width` |
| Cold build, vendored and git | 3.48 / 3.48 s and 3.58 / 3.68 s (min / median, 3 runs each) | `cargo clean`, then wasm-pack; registry and git caches warm |
| unicel on crates.io | absent (404) | `curl https://crates.io/api/v1/crates/{unicel,unicel-core}` |
| Bytes a cell: (a), (b) / (c) | 6 / 11–12 | buffer layout arithmetic |
| Largest scale step at 64 / 128 / 255 entries (viridis) | ΔE76 4.55 / 2.26 / 1.13 | `terminal/steps.mjs` |
| Atlas hits, braille scatter, 80 × 24, 64 / 255 / RGB | 3.6 / 0.9 / 0.6 % | `canvas/run.mjs`, 60 frames after 10 |
| Frame, braille scatter, 80 × 24: atlas / tint / `fillText` | 58.2–60.7 / 2.4–2.8 / 0.9 ms (median) | same |
| Frame, braille scatter, 320 × 120: atlas / tint / `fillText` | 1,130–1,337 / 32.8–34.5 / 16.1 ms | same |
| Frame, braille density, 320 × 120: atlas (100 % hits) / `fillText` | 46.1–48.0 / 20.7 ms | same |
| The same with `--disable-gpu` | 34.1 / 20.6 ms | `canvas/backend.mjs`, 3 runs |
| Frame, block heatmap, 320 × 120: rects / one image | 9.2–9.7 / 6.3 ms | same |
| Terminal full frame, heatmap 320 × 120: truecolour RGB / 256-colour | 777,760 / 204,601 B | `terminal/sgr.mjs` |
| viridis in xterm-256 | 15 colours, mean error 37.4, max 51.7 | same, 1,001 samples |
| Font additions: numbers and units / plus Latin | +7,216 / +10,052 B woff2 | `font/subset.py` |
| Digits apart at 8 / 11 px font, 1× | 1 / 7 px | `canvas/page.js` `digits` |

## Recommendation

1. **Core dependency.** Vendor unicel's `grid.rs`, `subcell.rs`,
   `palette.rs`, `bluenoise.rs`, `glyphs.rs` and the braille and dither
   helpers into glyphplot's core, plus `web/src/blocks.ts`, the font and
   `scripts/font.py`. Each file gets a header naming unicel, MIT, and the
   revision taken. Do not take `pixels.rs` or `meta.rs`. Adapt `mono.rs`'s
   checks into the chart contract. Revisit a shared `unicel-core` once both
   projects publish.
2. **Cell colour.** Keep unicel's `u32` code point, `u8` ink and `u8` back
   (255 = none). Lift the 64-colour cap to a per-chart palette of up to 255
   entries: theme roles first, then series colours, then scale ramps of at
   least 128 steps. The palette crosses the boundary as a table with a
   version number. Themes, paper and terminal downgrades remap the table.
3. **Renderer.** Drop the (code point, colour) atlas. Draw with `fillText`
   per changed cell, or a code-point-keyed atlas tinted by composite,
   whichever the owner's Chrome and Safari favour. Keep exact block fills
   and the one-image path for half-block heatmaps.
4. **Text.** Lay out in whole cells with integer gutters, greedy tick
   labels, draw order as z-order, and labels that clear the cells beneath.
   Extend the font with tier A, and tier B if Western European labels are
   in scope. Measure labels by East Asian Width in release.
5. **Resolution.** The cell comes from the font size at exactly 1:2 in whole
   device pixels; the box sets `cols × rows`; a resize lays out again,
   keeping the view in data coordinates.

## Requirements to propose

- REQ-CORE-010: The core MUST hold its own copies of unicel's `grid.rs`, `subcell.rs`, `palette.rs`, `bluenoise.rs` and `glyphs.rs`, each beginning with a comment that names unicel, its MIT licence and the source revision; the workspace MUST have no git dependencies. (v0.1.0)
- REQ-CORE-020: A check (`mise run vendor:check`) SHOULD print, for each vendored file, whether it differs from unicel at the revision its header names. (v0.2.0)
- REQ-GRID-010: A `Grid` MUST hold `cols × rows` cells row-major, each a `u32` code point, a `u8` ink index and a `u8` back index, with `BLANK = 0x20`, `WIDE_TAIL = 0` and `NO_BACK = 255`, as unicel's REQ-GRID-001 and REQ-GRID-010. (v0.1.0)
- REQ-GRID-020: `Grid::resize(cols, rows)` MUST give a grid of the new size with every cell `BLANK`, colour 0 and `NO_BACK`, and MUST panic on a zero size. (v0.1.0)
- REQ-COLOR-010: A chart's palette MUST hold 1 to 255 colours, indexed 0–254; a cell's ink and back MUST be indices into it or `NO_BACK`. (v0.1.0)
- REQ-COLOR-020: The palette MUST cross the boundary as `palette_ptr()` (255 × `u32` RGBA) with `palette_version()`, which MUST change exactly when an entry changes. (v0.1.0)
- REQ-COLOR-030: A continuous scale MUST be quantised to at least 128 palette entries; the contract test MUST check that no two neighbouring entries of a sequential scale differ by more than ΔE76 2.5. (v0.1.0)
- REQ-COLOR-040: A theme change (dark, light, paper, a page's theme) MUST repaint by swapping palette entries without calling the chart's frame; the shell's repaint MUST NOT call into wasm. (v0.2.0)
- REQ-COLOR-050: In one ink, a chart that encodes a value by colour MUST encode it by glyph density instead; the contract's one-ink check MUST pass for every chart with its fixture data. (v0.1.0)
- REQ-COLOR-060: The CLI MUST write truecolour by default and SHOULD take `--colors 256`, mapping each palette entry to its nearest xterm-256 colour once a palette, not per cell. (v0.3.0)
- REQ-RENDER-010: On a canvas, a frame's draw cost MUST NOT depend on the number of distinct (glyph, colour) pairs: a braille scatter of 320 × 120 cells with every cell a different colour MUST draw within 2 × the time of the same frame in one colour (headless Chrome, `mise run bench:render`). (v0.1.0)
- REQ-RENDER-020: Block, quadrant, sextant, octant and wedge glyphs MUST be filled as exact shapes from `blocks.ts`, never drawn from the font (unicel ADR-027). (v0.1.0)
- REQ-RENDER-030: A frame MUST redraw only the cells whose glyph, ink or back changed since the last frame drawn; a crosshair move over an unchanged chart MUST redraw at most the cells of the old and new crosshair and readout. (v0.1.0)
- REQ-RENDER-040: A frame whose cells carry back colours SHOULD be drawn as one image of `cols × 2·rows` pixels scaled without smoothing, with other glyphs over it (unicel REQ-MOUNT-080). (v0.2.0)
- REQ-FONT-010: The bundled font MUST cover unicel's ranges plus U+00A0–00BF, U+00D7, U+00F7, U+2007–200A, U+2010–2015, U+2018–201F, U+2026, U+202F, U+2030, U+2032–2033, U+2070–2071, U+2074–209F, U+20A1, U+20A3–20AF, U+20B1–20B5, U+20B8–20BA, U+20BC–20C0, U+2116, U+2122, U+2212, U+221E, U+2248, U+2260, U+2264–2265, U+0394, U+03A3, U+03A9, U+03BC, U+03C3, U+2713, U+2717, U+2605–2606 and U+FFFD, every glyph 0.6 em, and MUST be at most 32 KB. (v0.1.0)
- REQ-FONT-020: The font SHOULD also cover U+00C0–00FF and U+0100–017F. (v0.2.0)
- REQ-TEXT-010: A label MUST occupy whole cells: its width in cells is its East Asian Width (`unicode-width` or an equivalent table in release builds), wide glyphs written with `WIDE_TAIL`. (v0.1.0)
- REQ-TEXT-020: No two labels MUST share a cell, and no label MUST share a cell with data: a label clears the cells under it to the ground before it is drawn. The contract test MUST check this on every fixture at 80 × 24 and 320 × 120. (v0.1.0)
- REQ-TEXT-030: Tick labels MUST be at least one empty cell apart; where they would not fit, the layout MUST ask for fewer ticks before dropping any. (v0.1.0)
- REQ-TEXT-040: Number formatting MUST write U+2212 for minus and a plain space (one cell) for any group separator Intl or the formatter gives as U+2009, U+202F or U+00A0. (v0.1.0)
- REQ-LAYOUT-010: The shell MUST set the cell to `round(0.6 × font-size × devicePixelRatio)` device pixels wide and exactly twice that tall, and `cols × rows` to the whole cells that fit the content box; the remainder MUST be split evenly as padding. (v0.1.0)
- REQ-LAYOUT-020: A change of the element's box MUST lay the chart out again within one animation frame, keeping its view state in data coordinates; a box of width 0 MUST be ignored. (v0.1.0)
- REQ-LAYOUT-030: Every chart MUST draw a frame without panicking at every size from 1 × 1 to 320 × 120 with every fixture; below its minimum size it MUST drop parts in its declared order (title, legend, axes). (v0.1.0)
- REQ-LAYOUT-040: Each chart's meta MUST declare its minimum useful `cols × rows` with axes; the site's gallery and the contract test MUST use it. (v0.2.0)

## Decisions with a real alternative

**ADR (proposed): Vendor unicel's core rather than depend on it.**
Context: no unicel crate is published. glyphplot is meant for crates.io,
and crates.io rejects git dependencies. glyphplot also needs core changes
(palette size, resize). Options: (1) unicel publishes `unicel-core` first
and glyphplot depends on it. This needs a split, a name and a release in
unicel, and every core change waits for a unicel release. (2) A git
dependency pinned by revision. It works (+144 B), but it blocks publishing,
can only pin pushed commits, and needs the git CLI fetch setting and
network. (3) Vendor with provenance headers: about 1,240 lines, and drift
to be watched. Recommendation: (3), with a revisit once both projects have
published.

**ADR (proposed): A per-chart palette of up to 255 colours.**
Context: unicel caps a piece at 64 colours, and a continuous scale needs
more. Options: (a) 64; (b) a `u8` index into ≤ 255 per chart; (c) RGB a
cell. Measured: 64 steps band (ΔE76 4.55); 128 do not (2.26). Memory is
6 against 12 bytes a cell. The canvas cost is equal once the renderer's
key ignores colour. Terminal bytes are within 2 % of each other in
truecolour. (b) keeps themes, paper and the 256-colour mapping as table
swaps in the shell. Recommendation: (b).

**ADR (proposed): Replace the (code point, colour) atlas.**
Context: the atlas thrashes when glyph and colour are independent, at any
palette size (3.6 % hits with 64 colours). Options: unicel's atlas kept
for density-like charts only; a code-point atlas tinted by composite; direct
`fillText`. Recommendation: `fillText` (fastest in every measured case) or the tint, chosen on the owner's
Chrome and Safari in the first chart's phase. The atlas as it is should not
be kept.

**ADR (proposed): The cell from the font size, the grid from the box.**
Context: unicel fits a fixed grid to a box. A chart's text must stay
readable, so its cell has a floor. Options: fix `cols × rows` and scale the
cell (unicel's `fit`), or fix the cell from `font-size` and let `cols × rows`
follow the box. Recommendation: the second as the default. A `cols`/`rows`
pair stays possible for a fixed-size export.

**ADR (proposed): Extend the bundled font for chart text.**
Context: minus, `µ`, `…`, currency and accented letters are missing and fall
back to faces of other widths. Options: tier A (+7.2 KB), tiers A and B
(+10.1 KB), or no change with ASCII-only formatting (`-` for minus, `u` for
micro). Recommendation: A in v0.1.0, B in v0.2.0.

## Open questions for the owner

1. **Vendor or wait for `unicel-core`?** Options: vendor now (recommended);
   publish `unicel-core` from unicel first; a git dependency until then
   (which blocks publishing glyphplot). Recommendation: vendor now.
2. **Palette index or RGB?** Options: (b), a 255-entry palette
   (recommended); (c), RGB a cell. Recommendation: (b). Revisit if a chart
   must show three or more continuous scales at once.
3. **Which renderer?** Headless Chrome favours `fillText` per cell (16–21 ms
   full redraw at 320 × 120) over a tinted code-point atlas (33–47 ms).
   Options: measure both on your Chrome and Safari with a GPU (the bench
   page runs as it is) and pick; or take `fillText` now. Recommendation:
   measure first. It is a day's work in the first chart's phase.
4. **Font tiers?** Options: A only; A and B (recommended for v0.2.0);
   A, B and C, which needs the combining marks left out. Recommendation: A
   in v0.1.0, B in v0.2.0.
5. **The smallest font a chart may use.** The proxy says 11 px at 1×, and
   8 px from 2×. Options: a fixed floor of 11 CSS px; a floor that depends
   on the pixel ratio; no floor. Recommendation: please check digits by eye
   at 8–12 px on your screens. Until then, propose 11 px at 1× and 9 px at
   2× and above.
6. **Default height** of a chart with no CSS height (with agent 8). Options:
   a `rows` default (say 12); an aspect ratio; required CSS. Recommendation:
   a default of 12 rows, overridable.

## Sources

- The brief (`README.md`, "The briefs", topic 4) and the owner's decisions (`spec/adr/001` to `012`).
- unicel: `spec/SPEC.md` §3.1–3.5 (REQ-GRID, REQ-CANVAS, REQ-PAL, REQ-PIX),
  §6 (REQ-WASM), §7 (REQ-EL, REQ-FIT, REQ-MONO-004), §8 (REQ-MOUNT-050–110,
  REQ-PAINT-001, REQ-BLOCKS-001, REQ-FONT-001–030), §1.3; ADRs 003, 004,
  014, 016, 026, 027, 118, 120, 121.
- unicel code: `crates/unicel/src/grid.rs`, `subcell.rs`, `palette.rs`,
  `pixels.rs` (`SPREAD`, `tones`), `bluenoise.rs`, `util.rs`, `glyphs.rs`,
  `mono.rs`, `meta.rs`, `lib.rs`; `crates/unicel/Cargo.toml` (no
  `publish = false`); `crates/wasm/Cargo.toml:8` and
  `crates/cli/Cargo.toml:8` (`publish = false`); `crates/wasm/src/lib.rs`;
  `crates/unicel/tests/contract.rs:148` (2–64 colours);
  `crates/cli/src/paint.rs:167` (truecolour `sgr`); `web/src/mount.ts`
  (atlas, `size()`, `drawPixels`), `frame.ts`, `blocks.ts`, `fit.ts`,
  `element.ts` (`UNICODE_RANGE`); `scripts/font.py`;
  `docs/IMPLEMENTATION_PLAN.md:62`, `:582`.
- crates.io API, `https://crates.io/api/v1/crates/unicel` and
  `.../unicel-core`: 404 on 2026-10-18.
- Cargo reference, specifying dependencies (git dependencies and
  crates.io): https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
- MDN browser-compat-data, `api/ResizeObserverEntry.json`
  (`devicePixelContentBoxSize`: Chrome 84, Firefox 108, Safari not supported), read 2026-10-18:
  https://github.com/mdn/browser-compat-data/blob/main/api/ResizeObserverEntry.json
- Iosevka 34.9.0, `PkgTTF-Unhinted-IosevkaFixed-34.9.0.zip` (SHA-256 in
  unicel's `scripts/font.py`), read then from unicel's `target/font/` cache;
  `font/subset.py` now takes the release zip's path.
- Measurements: `discovery/bench/rendering/` (`core-size/`, `font/`,
  `canvas/`, `terminal/`), README there.
