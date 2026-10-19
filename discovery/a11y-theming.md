# Accessibility, theming and internationalisation

## Summary

- A chart's colours should cross the boundary as semantic indices (ground, ink, muted, rule, accent, series 1–8, up, down, scale steps) that the shell resolves to colours when it paints. A theme change is then a repaint, never a new frame, and a frame stays a pure function of data, options, view and `t`.
- Pages override the theme with CSS custom properties on the element, which cascade from the page's own `[data-theme]` and `prefers-color-scheme` rules. The JS object, the CLI, the Rust crate and the exports take the same tokens as a theme object. Without a ground, a chart picks its dark or light defaults by the luma of its computed text colour, as unicel's `mount` does (`web/src/mount.ts:95-101`).
- No well-known palette passes both bars this report proposes: 3:1 against its ground (WCAG 1.4.11) and a smallest CIEDE2000 difference of 15 between any two colours under normal vision and under simulated deuteranopia, protanopia and tritanopia. Okabe–Ito reaches 10.9 on dark and fails 3:1 on light in four of eight colours. Tableau 10 falls to 0.7 under deuteranopia. A constrained search finds 8-colour palettes that reach 15.6 to 21.0, so the bar can be met (see Measurements).
- Sequential: cividis keeps every 9-step neighbour at least 7.0 ΔE00 apart under all four visions. Viridis falls to 4.4 under protanopia. Diverging: ColorBrewer's light-middle scales vanish on a dark ground (RdBu's ends 1.39:1 and 1.40:1 on #131518), so a dark ground needs a dark-middle scale.
- One ink: series are told apart by dash patterns in braille dots, marker glyphs, fill shades and direct labels. The same cues satisfy WCAG 1.4.1 in colour.
- Accessibility: the host is a `role="figure"` with a generated summary on a `role="img"` drawing. It carries a visually hidden light-DOM `<table>` of the view, capped at 500 rows: 6 ms to build, against 523 ms for 10,000 rows. A `role="slider"` keyboard cursor, a pause control (WCAG 2.2.2, Level A), single-pointer alternatives to dragging (2.5.7, AA) and a rate-limited, opt-in live region complete the set.
- Internationalisation: the shell reads a locale pack (474–822 bytes) from `Intl` once and hands it to wasm, and Rust formats labels with it. ICU4X in a chart module costs 24 KB gzipped for numbers and 219 KB gzipped for dates. Labels are NFC, measured with unicode-width, and reduced to one scalar per grapheme cluster. Bidi reordering and Arabic joining come later.

## Scope and method

Read: the owner's brief (`PROMPT.md`, §10). From unicel (`../animate-unicode`): `spec/SPEC.md` §3.4, §7, §8, §10.6, §10.10 and the REQ-MONO-* requirements; `spec/ROADMAP.md` §7; ADRs 016, 026, 118, 119 and 121; `crates/unicel/src/{palette,pixels,meta,mono}.rs`; `web/src/{element,mount}.ts`; `site/e2e/a11y.spec.ts`; `site/src/site.css`; the palettes of `crates/unicel/src/pieces/data/{candlesticks,bar_chart,sparkline}.rs`. Web sources are listed under Sources and were read on 2026-10-18.

Built: the bench at `discovery/bench/a11y-theming/`, which is throwaway, with its README. It has Python scripts for contrast and colour-vision-deficiency (CVD) separation, a Rust tool for cell widths, a wasm build that weighs ICU4X, a Node script for `Intl`, and a Playwright run for the cost of a hidden table.

Machine for every measurement: Apple M5 Pro, 18 cores, 48 GB, macOS (Darwin 25.5.0), 2026-10-18. Toolchain: `mise exec`, giving Rust 1.97.1, Node 26.5.1 (ICU 78.3, Unicode 17.0, CLDR 48.0) and uv 0.12.23 with colour-science 0.4.6, numpy 2.5.4 and matplotlib 3.10.0.

Colour method:
- Contrast is WCAG 2.2's relative luminance and contrast ratio.
- CVD is simulated with the Machado, Oliveira and Fernandes 2009 matrices at severity 1.0 (`colour.blindness.matrix_cvd_Machado2009`), applied to linear sRGB and clipped.
- Differences are CIEDE2000 on CIELAB (D65) of the simulated colours.
- The palette computations are deterministic: the searches use fixed seeds, and repeated runs give the same numbers.
- Machado's model is considered weaker for tritanopia than Brettel 1997 (DaltonLens). The tritan figures are therefore the least reliable.

Not covered:
- Screen-reader behaviour was not tested in NVDA, JAWS, VoiceOver or TalkBack. The role and key recommendations rest on the specifications and the APG, not on AT runs, and those runs are the owner's manual checks.
- Forced-colours rendering was not tried on Windows.
- Safari and Firefox cost of the hidden table was not measured: that playwright-core's WebKit build is not installed.
- Sonification was surveyed only.
- The size of `unicode-normalization` and `unicode-bidi` in wasm was not measured.

## Themes

### What unicel does

- A piece has one palette and a ground, and `Env::paper` tells it that it is on a light ground (ADR-026; `crates/unicel/src/meta.rs` `palette`, `ground`).
- The palette is up to 64 `#rrggbb` colours. The grid holds a `u8` ink index and a `u8` back index a cell (REQ-GRID-001).
- `mount` decides paper from the ground's luma, or without a ground from the element's computed text colour: `0.2126 r + 0.7152 g + 0.0722 b < 128` (`web/src/mount.ts:95-101`, REQ-MOUNT-020).
- One ink draws in the canvas's `currentColor` or a `<pre>`'s colour, with no ground (REQ-MOUNT-010, REQ-MOUNT-090).
- The site's theme is `prefers-color-scheme` until the reader chooses; `data-theme` on `<html>` wins either way (REQ-THEME-001, REQ-THEME-010; `site/src/site.css:6-40`).
- The element's look survives page resets through inline style and custom properties: `--unicel-font`, `--unicel-line-height` and `--unicel-max-font` (ADR-118).

The data pieces glyphplot grows out of carry fixed dark-ground palettes. Candlesticks is `#4ac87a` up and `#e05a5a` down on `#0c0e12` (`crates/unicel/src/pieces/data/candlesticks.rs:26-27`). They tell series apart in one ink by authored means: hollow and solid candles, bracketed tabs, rows and labels (SPEC.md REQ-MONO-006 note).

### The theme model proposed

1. **Tokens.** These are the colours a chart uses:
   - `ground`: none by default, so the chart is transparent.
   - `ink`: text, meaning titles, tick labels and readouts.
   - `muted`: axes and ticks.
   - `rule`: gridlines.
   - `accent`: crosshair, selection and focus.
   - `series-1` to `series-8`.
   - `up` and `down`.
   - `scale`: the sequential stops.
   - `diverging`: the diverging stops.

   The grid cell's colour index names a token or a scale step, not a colour. This is the cell model unicel already has, an index into a palette, with the palette resolved late.
2. **Resolution, highest first:**
   - the `theme` property, a theme object;
   - CSS custom properties on the element or inherited (`--glyphplot-ground`, `--glyphplot-ink`, …, `--glyphplot-series-1` …);
   - the built-in dark or light default, chosen by the paper rule above.

   Because custom properties inherit, a page that switches `data-theme` or answers `prefers-color-scheme` in its own CSS re-themes every chart with no JS.
3. **When it re-resolves:**
   - when `prefers-color-scheme` changes (a `matchMedia` listener);
   - when `<html>`'s `class`, `style` or `data-theme` attribute changes (a `MutationObserver`);
   - when `theme` is set.

   Re-resolving clears the glyph atlas, as REQ-MOUNT-090 already does when the text colour changes, and repaints. It never asks wasm for a frame.
4. **Paper.** "Paper" stays unicel's meaning: dark marks on a light ground. A chart in paper mode is one ink, dark on light, for print (`@media print`) and for the exports' paper toggle (unicel REQ-EXPORT-020).
5. **Forced colours.** Under `forced-colors: active` the browser forces CSS colours but not a canvas's pixels. MDN lists the forced properties, and canvas content is not among them. So the chart must switch itself to one ink: `CanvasText` on `Canvas`, with `Highlight` for the accent.

The alternative of carrying RGB in the frame is agent 4's question. If each cell carries RGB from Rust, the theme must cross into wasm and a theme change needs a new frame. That is workable, but it couples the frame to the page's CSS. See Decisions.

### Contrast rules for data colours

WCAG 2.2 SC 1.4.11 (AA) asks 3:1 for "parts of graphics required to understand the content" against adjacent colours.
- Its Understanding page gives "each line in a graph" against its background as the case.
- Lines that rarely overlap need not contrast with each other. Pie slices that meet must.
- Heat maps may fall under the "essential" exception.
- Text labels that carry the same information relieve the graphic, and the text must then meet 1.4.3: 4.5:1 for small text.

SC 1.4.1 (A) forbids colour as the only means of telling things apart. That is the same rule as unicel's REQ-MONO-006, applied to colour mode too.

The rules proposed:
- Every default series colour is at least 3:1 against its default ground.
- Every default text token is at least 4.5:1.
- `rule` (gridlines) is exempt as decoration. unicel's site `--rule` is 1.27:1 (dark) and 1.20:1 (light), measured.
- Colours a page sets are used as given, and a debug build warns once when a series colour is under 3:1 against the resolved ground.

### Palettes measured

The full tables are in `discovery/bench/a11y-theming/results-palettes.md`, `results-optimise.md`, `results-anchored.md` and `results-updown.md`. Grounds: dark `#131518`, light `#f5f6f7` and paper `#f4efe2` (unicel's `site/src/site.css` lines 6, 13, 22), and white.

Categorical: smallest contrast against the grounds, and smallest ΔE00 between any two colours by vision.

| Palette | n | Min contrast, dark | Colours under 3:1, light `#f5f6f7` | Normal | Deutan | Protan | Tritan |
|---|---|---|---|---|---|---|---|
| Okabe–Ito (with black) | 8 | 1.15 (black) | 4 | 21.7 | 11.6 | 12.2 | 10.9 |
| Tableau 10 (d3) | 10 | 4.02 | 6 | 18.1 | **0.7** | **1.0** | 6.8 |
| Observable 10 (d3) | 10 | 3.63 | 7 | 18.0 | 2.6 | 6.0 | 8.2 |
| ColorBrewer Dark2 | 8 | 3.19 | 2 | 17.4 | 3.3 | 2.1 | 5.8 |
| Tol bright | 7 | 3.00 | 4 | 20.7 | 14.8 | 14.2 | 8.6 |
| Okabe–Ito lightness-fitted to 3:1 on light | 7 | 3.53 | 0 | 13.8 | **1.7** | 2.1 | 6.7 |
| Search, free, dark grounds | 8 | 3.24 | — | 22.0 | 21.0 | 21.3 | 21.2 |
| Search, free, light grounds | 8 | — | 0 (3.00 on paper) | 20.6 | 20.2 | 20.0 | 20.0 |
| Search held to Okabe–Ito hues, dark | 8 | 4.43 | — | 18.5 | 16.4 | 16.6 | 16.5 |
| Search held to Okabe–Ito hues, light | 8 | — | 0 (3.01 on paper) | 16.4 | 15.7 | 15.6 | 15.7 |
| Search, free, 10 colours, dark | 10 | 3.25 | — | 20.8 | 17.5 | 17.8 | 17.8 |
| Search, free, 10 colours, light | 10 | — | 0 | 19.3 | 17.3 | 17.2 | 17.4 |

What the table says:
- **Tableau 10 and Observable 10 are not CVD-safe.** Tableau's red `#e15759` and green `#59a14f` are 0.7 apart under deuteranopia. Observable's teal `#6cc5b0` and pink `#ff8ab7` are 2.6 apart.
- **Okabe–Ito is the best known palette on a dark ground but fails a light one.** On `#f5f6f7`, its orange, sky blue, yellow and reddish purple are 1.22:1 to 2.83:1.
- **Darkening Okabe–Ito to 3:1 does not fix the light ground.** Its orange and yellow become `#be8309` and `#9a9111`, which are 1.7 apart under deuteranopia. A light-ground palette has to be designed, not derived.
- **The free search reaches about 20 under every vision with 8 colours.** It does so by spreading lightness widely (`#feedb9` beside `#0b61f6`), which makes some series heavier than others.
- **Holding hues near Okabe–Ito and Oklab lightness to a band costs about 5 ΔE00.** The bands are 0.62–0.90 on dark and 0.38–0.64 on light, and the held search still clears 15.
- **Ten colours cap near 17 under CVD.** Beyond 8 series, colour cannot carry identity alone, and patterns must.

Searched palettes:
- Held to Okabe–Ito hues, dark: `#b29a02 #629cfb #72916d #c7f226 #abe6fd #f8d6b2 #e424a4 #b3b3b2`.
- Held to Okabe–Ito hues, light: `#9a8b0a #0784ff #628463 #746100 #324268 #6c2b28 #d2569b #5f6165`.

They meet the numbers. Their look has not been reviewed by a person, so they are candidates, not a choice (open question 1).

Sequential and diverging, 9 steps: smallest ΔE00 between neighbouring steps by vision, and the contrast of the first and last step.

| Scale | Normal | Deutan | Protan | Tritan | Dark ground, first / last | Light ground, first / last |
|---|---|---|---|---|---|---|
| viridis | 10.7 | 7.1 | **4.4** | 7.0 | 1.20 / 14.49 | 14.08 / 1.17 |
| cividis | 7.4 | 7.8 | 7.8 | 7.0 | 1.17 / 14.64 | 14.50 / 1.15 |
| magma | 11.3 | 8.8 | 6.2 | 10.2 | 1.15 / 17.37 | 19.37 / 1.03 |
| ColorBrewer Blues | 5.1 | 5.3 | 4.8 | 5.6 | 17.59 / 1.43 | 1.04 / 11.80 |
| RdBu (light middle) | 12.2 | 11.6 | 9.8 | 15.4 | **1.39 / 1.40** | 12.16 / 12.12 |
| PuOr (light middle) | 13.0 | 12.3 | 12.2 | 9.1 | 2.20 / 1.06 | 7.67 / 16.00 |
| Blue–dark grey–orange (Oklab, mid L 0.36) | 10.1 | 10.2 | 10.1 | 8.4 | 10.59 / 10.24 | 1.60 / 1.65 |
| Blue–light grey–orange (Oklab, mid L 0.96) | 12.6 | 11.5 | 12.2 | 12.3 | 2.43 / 2.90 | 6.95 / 5.82 |

Notes on the scales:
- All four sequential scales are monotonic in L* under all four visions.
- Each half of every diverging scale is monotonic in L* under all four visions.
- The two ends of every diverging scale differ by at least 13.9 under every vision. BrBG's 13.9 under protanopia is the lowest.
- The proposed diverging scales are `#90cbfe … #3d3d3d … #feb263` (dark) and `#0c54a0 … #f2f2f2 … #9a490c` (light). The full stops are in `results-palettes.md`.

A sequential scale's low end should sit nearest the ground's lightness, so that more is more ink. That means viridis or cividis forward on a dark ground and reversed on a light one. The low end then has little contrast with the ground (about 1.2:1), which the heat-map exception of 1.4.11 allows. A cell holding a value, even the lowest, still differs from an empty cell, which is a space.

Candles, up against down:

| Pair | Normal | Deutan | Protan | Tritan | On `#f5f6f7`, up / down |
|---|---|---|---|---|---|
| unicel `#4ac87a` / `#e05a5a` | 69.0 | **9.3** | 24.0 | 69.3 | 1.97 / 3.36 |
| Okabe–Ito `#009e73` / `#d55e00` | 54.4 | 20.5 | 18.2 | 62.7 | 3.16 / 3.57 |
| blue `#56b4e9` / orange `#e69f00` | 53.6 | 54.3 | 50.5 | 53.6 | 2.13 / 2.08 |

Green and red fail deuteranopes as unicel's pair shows. Whatever colours the owner picks, up and down must also differ by fill, hollow against solid, which unicel's candlesticks already do. Red means up in mainland China and Taiwan markets, the reverse of US convention **(unverified)**, so `up` and `down` are tokens, never hard-coded.

## One ink

unicel's rule (REQ-MONO-006) is that where colour carries meaning, one ink must carry it another way. For a chart, colour carries series identity, state (selected, up or down) and magnitude (heat maps). The proposed rules:

| Meaning | One-ink carrier | Notes |
|---|---|---|
| Line series 1–4 | Dash pattern in braille dots along x: solid; 3 on 1 off; 1 on 1 off; 3 on, 1 off, 1 on, 1 off | Counted in absolute dot columns from the scale's origin, so a pan by whole dots shifts the pattern with the data and dashes do not crawl. |
| Line series 5–8 | Patterns 1–4 again, plus a marker per series | Markers from Geometric Shapes (U+25A0–25FF, unicel's `extended` charset): `● ○ ■ □ ▲ △ ◆ ◇`, at most one a cell. A marker replaces the braille cell it sits in. |
| Bars and areas | Fill by shade: `█ ▓ ▒ ░`, or octant and sextant dot densities | `░▒▓` are in unicel's `basic` set (U+2500–259F). |
| Heat maps | Dot density, 0 to 8 dots of a braille or octant cell, giving 9 levels; or ` ░▒▓█`, giving 5 | unicel's `mono::ink` already counts dots a glyph (`crates/unicel/src/mono.rs:43-66`). |
| Up / down candles | Hollow / solid | As unicel's candlesticks. |
| Selection, cursor | Inverse cells (ink and ground swapped), or brackets | As REQ-MONO-006's examples. |
| Diverging sign | Glyph: above and below a baseline, or `+`/`−` markers | Lightness alone cannot carry a sign. |
| Series names | Legend key showing each series' pattern or marker; direct labels at the last visible point when there is room | Direct labels also satisfy 1.4.1 in colour. |

Measured limits that bound these rules:
- Braille gives 2 × 4 dots a cell (unicel REQ-CANVAS-020). A dash period under 4 dots reads as noise on steep lines **(unverified, needs the owner's eye)**.
- Four patterns times eight markers is 32 distinguishable series. Past 8, small multiples are the better answer.

The contract test should hold a 4-series fixture's one-ink frame to four distinct glyph signatures.

## Accessibility

### Requirements from WCAG 2.2 that bite a chart

| SC | Level | What it asks of glyphplot |
|---|---|---|
| 1.1.1 Non-text content | A | A text alternative: the summary label and the table. |
| 1.3.1 Info and relationships | A | The data's structure available to AT: a real `<table>` with headers. |
| 1.4.1 Use of colour | A | Series distinguished without colour (One ink above). |
| 1.4.3 / 1.4.11 Contrast | AA | 4.5:1 text, 3:1 marks (Themes above). |
| 1.4.13 Content on hover or focus | AA | Readouts dismissible (Escape), hoverable, persistent. A readout pinned to a corner is hoverable by construction; one that follows the pointer is not. |
| 2.1.1 Keyboard | A | Every interaction without a pointer. |
| 2.2.2 Pause, stop, hide | **A** | A streaming chart is "auto-updating information", which needs a mechanism to pause, stop or hide it or to control its frequency, with no 5-second exemption. The Understanding page advises that real-time data resume to the current state, or say it is delayed. |
| 2.4.7 Focus visible | AA | A visible focus ring on the chart. |
| 2.5.7 Dragging movements | **AA** | Pan and brush by drag need a single-pointer alternative without dragging. Keyboard does not satisfy it. |
| 4.1.3 Status messages | AA | Announcements by live region without moving focus. |

Chartability (Elavsky et al.) marks 14 heuristics critical. Among them are "No table", "Content is only visual", "No title, summary, or caption", "Interaction modality only has one input type", "Navigation and interaction is tedious" and "Data density is inappropriate". The last two argue against a screen reader stepping through a million points one at a time.

### The element's role and label

Options:
- **`role="img"`, as unicel (REQ-EL-050).** A focused `img` cannot contain navigable parts: its children are presentational. That is fine for a static chart and wrong for an interactive one.
- **`role="graphics-document"` (Graphics-ARIA 1.0, Recommendation 2018-10-02).** Its definition names charts. AT support for its children (`graphics-object`, `graphics-symbol`) is uneven **(unverified)**.
- **`role="application"`.** This hands all keys to the page and removes the screen reader's own reading keys. It is the one Chartability warns of under "Controls override AT controls".
- **Proposed: the host has `role="figure"` and an accessible name from `label`.** If `label` is absent, the name is "<chart type>: <series names>". Inside it:
  - The drawing (canvas or `<pre>`) is `role="img"`, with an `aria-label` the chart writes. The label gives the chart type, the series, each axis's title and visible domain formatted for the locale, and the latest value for a time series. It is rewritten at most once a second.
  - A visually hidden `<table>` of the data in view.
  - A keyboard cursor (next subsection).
  - An opt-in live region.

### Keyboard navigation of points

The APG grid pattern fits the table, and a plain `<table>` is already navigable with a screen reader's table commands. The drawing needs a cursor. Its proposed form:
- A transparent element over the chart with `role="slider"`, `tabindex="0"` (one tab stop), `aria-valuemin` and `aria-valuemax` set to the x domain in view, and `aria-valuenow`.
- `aria-valuetext` reads the cursor's x and every series' value there, for example "10:32, Price 104.20, Volume 1,200". It reads at most four series, then "and N more".
- Slider keys are the APG's: ←/→ and ↓/↑ move to the previous or next point, Home and End go to the first or last point in view, and PageUp and PageDown move a tenth of the view.
- Further keys: `+` and `−` zoom around the cursor, Shift+←/→ pans, `0` resets, Escape hides the readout (1.4.13) and Space pauses a stream (2.2.2). They are listed in `aria-keyshortcuts`.
- Its focus ring is the chart's ring (`:focus-visible`, 2 px in `accent`, at least 3:1 against the ground). The cursor is also drawn in the grid as the crosshair.
- Series are switched by the legend, whose entries are toggle buttons (`aria-pressed`), not by arrow keys.

The slider was chosen because a widget role puts screen readers into focus mode, so arrows reach the chart, and because `aria-valuetext` is spoken on change without a live region. Highcharts' module also gives keyboard navigation and a "view as data table" (its overview page). Olli renders an ARIA tree view of a chart's structure for Vega, Vega-Lite and Observable Plot (umwelt-data/olli README). A tree is the better model for multi-pane charts later. AT behaviour of the slider in NVDA, JAWS and VoiceOver is the owner's check.

### The data table

**Where it lives.** Light DOM is the recommendation. unicel's element uses no shadow root (ADR-118 rejected one), and ARIA IDREFs (`aria-describedby`, `aria-labelledby`) do not cross a shadow boundary. Chromium's "Reference Target" is the fix in progress (blink-dev Intent to Ship; Nolan Lawson 2022), and it is not yet everywhere. A hidden table inside a shadow root is still read in document order, but the page cannot point at it. `aria-describedby` to the table is wrong too: a description is flattened to a string.

**Size, measured.** The cost is to build a visually hidden table of three columns and force layout, in headless Chromium 156:

| Rows | 100 | 250 | 500 | 1,000 | 2,000 | 5,000 | 10,000 |
|---|---|---|---|---|---|---|---|
| ms, median of 3 | 1.5 | 3.3 | 6.0 | 13.0 | 32.7 | 129.8 | 523.7 |

The cost grows faster than the rows past 1,000. 100,000 rows did not finish in 5 minutes. A cause in the `white-space: nowrap; clip-path` hiding style is possible **(unverified)**.

**Rules proposed:**
- The table holds the view, not the data.
- It holds at most 500 rows: one page, about 6 ms. If the view has more points, the rows are the chart's own downsampled points, with each bucket's minimum and maximum kept, and a caption saying "500 of 1,000,000 points; zoom in for more".
- A "show data" control makes the table visible, so sighted readers get it too (Chartability, "Content is only visual").
- While streaming, the table is rebuilt at most once a second, and never while it holds focus.
- The CLI's `--table` prints the same table as text.

### Live region for streaming

Highcharts' `accessibility.announceNewData` has a `minAnnounceInterval` defaulting to 5000 ms. Its docs warn that frequent announcements do not help and suggest batching.

Proposed:
- Off by default.
- An `announce` attribute turns it on, at an interval in seconds with a floor of 5.
- An `aria-live="polite"` region reads the latest value and the change since the last announcement.
- It is silent while the chart is paused and while the cursor has focus, since the slider speaks then.

### Reduced motion, pause and dragging

- **Reduced motion.** Under `prefers-reduced-motion: reduce`, the chart draws no animated transitions: no eases, no inertial pan, no smooth scrolling of a streaming window. The view jumps. A `motion` attribute overrides this, as unicel's does (REQ-MOUNT-120). This is about transitions only: new data still draws.
- **Pause.** A `paused` property and attribute, Space on the cursor, and a visible pause control when `controls` is set. Data that arrives while paused is kept and not drawn. Resuming jumps to the live view, as the 2.2.2 Understanding page advises for real-time data.
- **Dragging alternatives.** A small control row: `[−] [+] [reset] [◀] [▶]`, shown on focus and hover or always. Brushing also works as click to start, click to end. Both are needed in v0.1.0 because the chart is interactive from its first release and 2.5.7 is AA.

### Sonification

Highcharts lists sonification ("audio charts") as a feature of its accessibility module. It is worth a module of its own, loaded on demand. It is deferred: nothing in v0.1.0 needs it.

## Internationalisation

### Number and date formats

The options:
- **Intl in the shell.** It costs 0 bytes and every browser has it, but it is not available to the CLI, the Rust crate or deterministic snapshots.
- **ICU4X in Rust.** It works everywhere, at a cost in size.
- **A locale pack.** The shell reads the symbols, digits, names and patterns with `formatToParts` once, and Rust formats with them.

Measured:

| Build | Raw bytes | gzip -9 bytes | Added (gzip) |
|---|---|---|---|
| Hand-written integer formatting with ASCII grouping (`std` `to_string`) | 17,140 | 7,485 | — |
| ICU4X `icu_decimal` 2.3.0, compiled data, any locale at run time | 81,381 | 31,500 | +24,015 |
| ICU4X `icu_datetime` 2.x, `YMD::medium`, compiled data | 796,948 | 226,664 | +219,179 |

How these were measured: `cargo build --release --target wasm32-unknown-unknown` with unicel's release profile (`opt-level = "s"`, LTO, one codegen unit, `panic = "abort"`, strip), `discovery/bench/a11y-theming/sizes.sh`, two runs with identical sizes. There was no `wasm-opt` pass, so wasm-pack's output would be somewhat smaller. ICU4X's outputs were checked to be correct (`cargo test --features icu`, `--features icu_dt`): en-IN gave `-12,34,567`, ar-EG gave `؜-١٬٢٣٤٬٥٦٧`, and th-TH gave Buddhist year 2569.

| Locale pack from Intl | Value |
|---|---|
| JSON size, 11 locales | 474–822 bytes |
| Build time, cold | 4.23 ms a locale (mean of 11, one run) |
| 20 tick labels, cached `Intl.NumberFormat` | 4.7–6.2 µs a frame (3 runs × 2 locales, 10,000 frames each) |
| A new `Intl.NumberFormat` per label | 19.5 µs each |

The recommendation is the locale pack:
- Dates by ICU4X would almost certainly exceed any per-module budget, and a number formatter by ICU4X costs as much as a small chart.
- Rust keeps formatting, so frames stay pure functions of their inputs and snapshots repeat.
- Per-label calls out to JS are avoided.

The pack holds:
- the decimal and group separators, grouping sizes (en-IN groups 2-2-3), the minus sign (fa-IR uses U+2212), the percent pattern and the ten digits;
- short and long month names and short weekday names;
- date and time patterns from `formatToParts`;
- the text direction.

It must be read with the Gregorian calendar forced (`-u-ca-gregory`). Without it, fa-IR's month names come back from the Persian calendar, misaligned with Gregorian months, which the bench shows in `results-localepack.md`. Without a pack (the CLI, Rust, tests), formatting is a fixed, ASCII, en-US-like default. The CLI may later take `--locale` with packs generated at build time.

Locale resolution: the element's `locale` attribute, else the nearest `[lang]` ancestor, else `navigator.language`.

### Widths, digits and clusters in cells

Measured with unicode-width 0.2.2 (unicel's) and unicode-segmentation 1.13.2, on `Intl`'s own output (`results-widths.md`):

| Label | Code points | Graphemes | Σ width a code point | Notes |
|---|---|---|---|---|
| `-1,234,567.891` (en-US) | 14 | 14 | 14 | |
| fr-FR | 14 | 14 | 14 | The group separator is U+202F (narrow no-break space), width 1 |
| ar-EG `؜-١٬٢٣٤٬٥٦٧٫٨٩١` | 15 | 15 | 14 | Leads with U+061C (Arabic letter mark), width 0 |
| ar, he-IL, fa-IR | 15 | 15 | 14 | Lead with U+200E (left-to-right mark), width 0 |
| ar-EG currency | 21 | 21 | 18 | U+061C and two U+200F marks |
| Arabic-Indic `٠…٩`, Extended `۰…۹` | 10 | 10 | 10 | Each digit is one cell |
| `10月` (ja-JP, zh-CN month) | 3 | 3 | 4 | CJK is wide: two cells |
| zh-CN date `2026年10月18日 14:05` | 17 | 17 | 20 | |
| `東京証券取引所` | 7 | 7 | 14 | |
| `Café` precomposed / `Cafe◌́` decomposed | 4 / 5 | 4 / 4 | 4 / 4 | U+0301 has width 0 but would take a cell of its own |
| 👨‍👩‍👧 (ZWJ family) | 5 | 1 | 6 | String width 2; drawn a code point a cell, it is three people and two joiners |
| 🇯🇵 (flag) | 2 | 1 | 2 | Two regional-indicator letters |
| ☀︎ with VS16 | 2 | 1 | 1 | String width 2: the emoji presentation is wide |
| हिन्दी | 6 | 2 | 5 | Conjuncts; one code point a cell breaks them |

So:
- Every digit system `Intl` produces measures one cell a digit. Arabic-Indic digits are no harder to lay out than ASCII.
- The bidi marks `Intl` emits have width 0. They must not take a cell; they are only meaningful to a bidi algorithm.
- A grid that holds one code point a cell (REQ-GRID-001) cannot hold a multi-scalar grapheme.

The rules proposed:
- Labels are NFC-normalised. The shell does it with `String.prototype.normalize`, so no normalisation tables go into wasm. Normalisation turns `e` + U+0301 back into `é`.
- Zero-width code points take no cell. Bidi marks are applied first, then dropped.
- A grapheme cluster of several scalars is drawn as its first scalar, wide if that is wide (`set_wide`, REQ-GRID-060). The accessible text keeps the original string.

The bundled font covers `basic` and `extended` only (REQ-CHAR-020). U+00A0, U+202F, U+2212, the Arabic separators (U+066A–066C) and the Arabic-Indic digits fall through to system fonts. On a canvas, the atlas clips each glyph to its slot (REQ-MOUNT-070), so layout holds. In a `<pre>` their advances may not be 0.6 em **(unverified)**. That is agent 4's font question.

### Right to left

The grid is a visual left-to-right array.
- Hebrew labels are readable once reordered into visual order by the Unicode bidirectional algorithm (UAX #9; Rust: `unicode-bidi`).
- Arabic letters drawn one a cell lose their joining forms, unless they are mapped to Arabic Presentation Forms-B (U+FE70–FEFF), which covers the basic letters only.
- Indic conjuncts and Thai marks need shaping that a cell grid does not do.

Proposed:
- v0.1.0 reorders nothing and documents that labels in RTL and complex scripts are drawn in logical order, one code point a cell. The table and summary carry them correctly, because they are DOM text.
- v0.2.0 reorders RTL runs by UAX #9 and moves the y axis and legend to the right for an RTL locale.
- Whether a time axis runs right to left in RTL locales is the owner's call (open question 4). No convention was verified.

## Measurements

All on Apple M5 Pro, 18 cores, 48 GB, macOS (Darwin 25.5.0), 2026-10-18, through `mise exec`. Each command is run from the repository root unless noted. The bench README has every rerun line.

| What | Command | Repetitions | Result |
|---|---|---|---|
| Palette contrast and CVD ΔE00 | `uv run --no-project --with colour-science==0.4.6 --with numpy==2.5.4 --with matplotlib==3.10.0 python -W ignore discovery/bench/a11y-theming/palettes.py` | deterministic (no randomness); run once | `results-palettes.md`; tables above |
| Free palette search | same, `optimise.py` | 6 restarts × 4,000 steps, seed 1; 2 min 24 s | `results-optimise.md` |
| Okabe–Ito-anchored search | same, `anchored.py` | 5 restarts × 6,000 steps, seed 2; 1 min 18 s | `results-anchored.md` |
| Up/down pairs | same, `updown.py` | deterministic | `results-updown.md` |
| Label widths | `node discovery/bench/a11y-theming/intl.mjs \| discovery/bench/a11y-theming/target/release/widths` | deterministic | `results-widths.md` |
| wasm size of formatting | `cd discovery/bench/a11y-theming && bash sizes.sh` | 2 builds, identical | 7,485 / 31,500 / 226,664 bytes gzip -9 |
| Locale pack and Intl label cost | `node discovery/bench/a11y-theming/localepack.mjs` | 3 runs of 10,000 frames × 2 locales | 4.7–6.2 µs for 20 labels; pack 474–822 bytes |
| Hidden table cost | `ROWS=… BROWSERS=chromium PW=…/playwright-core/index.js node discovery/bench/a11y-theming/table-run.mjs` (headless Chromium 156.0.8078.4 via unicel's playwright-core) | 3 a size | `results-table.md`; table above |

Numbers in this report with no measurement behind them are marked **(unverified)**.

## Recommendation

**Theme model.**
- Semantic colour indices in the frame, resolved by the shell.
- CSS custom properties first, inherited from the page.
- A theme object with the same token names for the JS API, the CLI, the crate and the exports.
- Dark or light defaults chosen by the text colour's luma.
- One ink for paper, print and forced colours.

**Defaults.**
- 8 categorical colours a ground, held to 3:1 and to 15 ΔE00 under all four visions. A candidate pair is in the bench, pending the owner's look.
- cividis as the sequential scale.
- A blue–orange diverging scale with its middle on the ground's side.
- Up and down told apart by fill as well as colour.

**v0.1.0 must have:**
- the tokens and custom properties;
- the default palettes and the contrast and CVD checks in the test suite;
- one-ink rules for line, bar and candle (dash patterns, markers, fills, hollow and solid);
- `role="figure"`, the summary label and the hidden light-DOM table capped at 500 rows;
- the slider cursor with its keys, the focus ring and the readout pinned so it is hoverable and dismissible;
- pause for streams (2.2.2), the drag alternatives (2.5.7) and reduced motion;
- the locale pack, locale resolution, NFC and cell-width rules;
- an axe scan in light and dark.

**v0.1.0 may defer:**
- to v0.2.0: forced-colours handling, print paper, the diverging default, the visible "show data" table, the opt-in live region, RTL reordering and axis mirroring, and the CLI's `--table` and `--locale`;
- to v0.3.0 or later: Arabic joining, an Olli-style tree for multi-pane charts, and sonification.

## Requirements to propose

Theme:
- REQ-THEME-010 (v0.1.0): A frame's colour indices MUST name theme tokens or scale steps, not colours; the shell MUST resolve them when it paints, and a theme change MUST repaint without calling the chart's frame function.
- REQ-THEME-020 (v0.1.0): The element MUST read `--glyphplot-ground`, `--glyphplot-ink`, `--glyphplot-muted`, `--glyphplot-rule`, `--glyphplot-accent`, `--glyphplot-series-1` … `--glyphplot-series-8`, `--glyphplot-up`, `--glyphplot-down` and `--glyphplot-scale` from its computed style, each falling back to the default set; a `theme` property set to an object with the same names MUST win over them.
- REQ-THEME-030 (v0.1.0): Without a ground, the chart MUST draw no ground and MUST use the light default set when its computed `color` has luma `0.2126 r + 0.7152 g + 0.0722 b < 128`, the dark set otherwise.
- REQ-THEME-040 (v0.1.0): The element MUST re-resolve its theme and repaint when `prefers-color-scheme` changes, when `<html>`'s `class`, `style` or `data-theme` changes, and when `theme` is set, without reloading its module.
- REQ-THEME-050 (v0.1.0): The JS API, the CLI (`--theme <file.json>`), the Rust crate and the SVG and ANSI exports MUST accept a theme object with REQ-THEME-020's token names.
- REQ-THEME-060 (v0.1.0): Every default series, `up`, `down` and `accent` colour MUST have a WCAG 2.2 contrast ratio of at least 3:1 against its default ground, and `ink` and `muted` at least 4.5:1.
- REQ-THEME-070 (v0.1.0): Each default categorical palette MUST hold 8 colours whose smallest pairwise CIEDE2000 difference is at least 15 under normal vision and under deuteranopia, protanopia and tritanopia simulated by Machado 2009 at severity 1.0 on linear sRGB.
- REQ-THEME-080 (v0.1.0): The default sequential scale sampled at 9 steps MUST be monotonic in L* and have neighbouring steps at least 5 ΔE00 apart under each of the four visions, and its low end MUST be the end nearer the ground's lightness.
- REQ-THEME-090 (v0.2.0): The default diverging scale MUST run blue to orange with its middle on the ground's side (dark on a dark ground, light on a light one), each half monotonic in L* under the four visions, and both ends at least 3:1 against the ground.
- REQ-THEME-100 (v0.1.0): In a debug build, a page-set series colour under 3:1 against the resolved ground MUST log one console warning naming the token and the ratio, and MUST still be used.
- REQ-THEME-110 (v0.2.0): Under `forced-colors: active` the element MUST draw in one ink in `CanvasText` on `Canvas`, with the accent in `Highlight`.
- REQ-THEME-120 (v0.2.0): Under `@media print` the element SHOULD draw in one ink, dark on a light ground (paper).
- REQ-THEME-130 (v0.1.0): Up and down candles and bars MUST differ by fill (hollow and solid) as well as by colour, in colour and in one ink.

One ink:
- REQ-MONO-010 (v0.1.0): In one ink, line series 1–4 MUST draw with dash patterns solid, 3-on-1-off, 1-on-1-off and 3-on-1-off-1-on-1-off, counted in dot columns from the x scale's origin, so a pan of n whole dots shifts the pattern by n dots.
- REQ-MONO-020 (v0.1.0): In one ink, series 5–8 MUST repeat patterns 1–4 and add a marker per series from `● ○ ■ □ ▲ △ ◆ ◇`, at most one marker a cell.
- REQ-MONO-030 (v0.1.0): In one ink, bar and area series MUST fill with `█ ▓ ▒ ░` by series, and heat maps MUST shade by dots lit (0–8 a cell).
- REQ-MONO-040 (v0.1.0): The legend MUST draw each series' one-ink key (pattern, marker or fill) beside its name, in colour and in one ink.
- REQ-MONO-050 (v0.1.0): The contract test MUST check that every chart's one-ink frame of a 4-series fixture gives each series a distinct set of glyphs.
- REQ-MONO-060 (v0.2.0): With up to 4 series and room, a line chart SHOULD label each series at its last visible point.

Accessibility:
- REQ-A11Y-010 (v0.1.0): The element MUST have `role="figure"` and an accessible name from `label`, else "<chart type>: <series names>"; its drawing MUST have `role="img"` and an `aria-label` giving the chart type, the series names, each axis's title and visible domain in the resolved locale, rewritten at most once a second.
- REQ-A11Y-020 (v0.1.0): The element MUST hold, in light DOM and visually hidden, a `<table>` with a caption, `th scope="col"` headers and a row header a row, of the data in the current view, at most 500 rows (downsampled with each bucket's minimum and maximum kept), with the caption stating how many points the view holds when it holds more.
- REQ-A11Y-030 (v0.1.0): While data streams, the table MUST be rebuilt at most once a second and never while focus is inside it.
- REQ-A11Y-040 (v0.1.0): An interactive chart MUST have exactly one tab stop, a `role="slider"` cursor whose `aria-valuetext` gives the cursor's x and each series' value there (up to 4, then "and N more"), moved by ←/→/↑/↓ (a point), Home/End (first or last in view) and PageUp/PageDown (a tenth of the view).
- REQ-A11Y-050 (v0.1.0): On the focused cursor, `+`/`−` MUST zoom around it, Shift+←/→ MUST pan, `0` MUST reset the view, Escape MUST hide the readout and Space MUST pause or resume a stream; the keys MUST be listed in `aria-keyshortcuts`.
- REQ-A11Y-060 (v0.1.0): Every pan, zoom and brush done by dragging MUST also be possible with single clicks or taps (zoom, reset and step controls, and click-click brushing).
- REQ-A11Y-070 (v0.1.0): The hover and focus readout MUST stay at a fixed place in the chart (not under the pointer), stay until the pointer or focus leaves, and hide on Escape.
- REQ-A11Y-080 (v0.1.0): A chart receiving streamed data MUST support `paused` (property and attribute); while paused it MUST keep appended data without drawing it, and on resume MUST show the live view.
- REQ-A11Y-090 (v0.1.0): Under `prefers-reduced-motion: reduce` and without `motion`, view changes MUST jump with no animated transition.
- REQ-A11Y-100 (v0.1.0): The focused chart MUST show a `:focus-visible` outline at least 2 px wide in `accent`, at least 3:1 against the ground.
- REQ-A11Y-110 (v0.1.0): An axe-core scan of a page with every chart, in the light and dark themes, MUST find no serious or critical violations.
- REQ-A11Y-120 (v0.2.0): With `announce="<seconds>"` (at least 5), a polite live region MUST announce the latest value at most once per interval, and MUST be silent while paused or while the cursor has focus; without it there MUST be no live region.
- REQ-A11Y-130 (v0.2.0): A "show data" control MUST make the table visible.
- REQ-A11Y-140 (v0.2.0): `glyphplot <chart> --table` MUST print the data table as text.
- REQ-A11Y-150 (v0.4.0): Sonification of a series MAY be offered as a module of its own, loaded only when asked for.

Internationalisation:
- REQ-I18N-010 (v0.1.0): The element's locale MUST be its `locale` attribute, else the `lang` of its nearest ancestor with one, else `navigator.language`.
- REQ-I18N-020 (v0.1.0): The shell MUST build a locale pack (decimal and group separators, grouping sizes, minus sign, percent pattern, the ten digits, month and weekday names, date and time patterns, direction) from `Intl` with the Gregorian calendar, once per locale, and pass it to wasm; chart modules MUST NOT link ICU4X.
- REQ-I18N-030 (v0.1.0): Without a pack, formatting MUST use `.` as decimal, `,` as group, groups of 3, ASCII digits and `-`.
- REQ-I18N-040 (v0.1.0): Label layout MUST count cells with unicode-width 0.2 a code point and draw width-2 glyphs with `set_wide`.
- REQ-I18N-050 (v0.1.0): Labels MUST be NFC-normalised before layout; zero-width code points MUST take no cell; a grapheme cluster of several scalars MUST be drawn as its first scalar; the accessible name, summary and table MUST keep the original text.
- REQ-I18N-060 (v0.2.0): Labels with right-to-left runs MUST be reordered by UAX #9 before layout, and an RTL locale MUST put the y axis and the legend on the right.
- REQ-I18N-070 (v0.2.0): The bundled font SHOULD cover U+00A0, U+202F, U+2212, U+066A–066C, U+0660–0669 and U+06F0–06F9.
- REQ-I18N-080 (v0.4.0): Arabic labels MAY be drawn in contextual forms through Arabic Presentation Forms-B.

## Decisions with a real alternative

1. **Colours in the frame: semantic indices against RGB.**
   - Context: agent 4 weighs a palette index against RGB a cell; theming depends on it.
   - Options:
     - (a) Indices naming tokens and scale steps, resolved by the shell. A theme change is a repaint, frames are independent of the page's CSS, and memory stays a byte a cell.
     - (b) RGB in the frame. Continuous colour is free, but theme crosses into wasm and a theme change needs a frame.
     - (c) Indices with a larger palette (256 steps of scale).
   - Recommendation: (a), with (c)'s 256 scale steps if agent 4 shows heat maps need more than the token count.
2. **Theme overrides: custom properties against a theme object.**
   - Options:
     - (a) CSS custom properties, with the same tokens as an object for non-CSS surfaces.
     - (b) An object or attribute only: simpler to test, but it ignores the page's own dark and light rules.
     - (c) Named presets only (`theme="dark"`).
   - Recommendation: (a). It is unicel's pattern (ADR-118's custom properties) and it follows `data-theme` with no code.
3. **Default categorical palette.**
   - Options:
     - (a) Okabe–Ito as published: known and cited, but it fails 3:1 on light grounds and reaches only 10.9 ΔE00 under CVD.
     - (b) Tableau 10 or Observable 10: familiar, but 0.7 and 2.6 under deuteranopia.
     - (c) A palette a ground, searched to meet REQ-THEME-060 and REQ-THEME-070, held near Okabe–Ito's hues.
   - Recommendation: (c), with the owner choosing among candidates by eye.
4. **The element's semantics.**
   - Options:
     - (a) `role="img"` only, as unicel.
     - (b) `role="application"`.
     - (c) `role="graphics-document"`.
     - (d) `role="figure"` holding a `role="img"` summary, a hidden table and a `role="slider"` cursor.
   - Recommendation: (d), checked by the owner in NVDA, JAWS and VoiceOver.
5. **Where the data table lives.**
   - Options:
     - (a) Light DOM, visually hidden.
     - (b) Inside a shadow root.
     - (c) A page-provided table referenced by `aria-describedby`.
   - Recommendation: (a). IDREFs do not cross shadow roots yet, and a description flattens a table.
6. **Number and date formatting.**
   - Options:
     - (a) A locale pack from `Intl`, with Rust formatting.
     - (b) Rust calling `Intl` for each label.
     - (c) ICU4X in each module (+24 KB gzip for numbers, +219 KB for dates).
   - Recommendation: (a).
7. **Multi-scalar graphemes in labels.**
   - Options:
     - (a) Reduce each to its first scalar.
     - (b) Widen the grid to hold a string a cell for labels.
   - Recommendation: (a) for v0.1.0. (b) breaks REQ-GRID-001's typed-array views.

## Open questions for the owner

1. **Which default palette?**
   - Options: (a) the Okabe–Ito-anchored candidates in `results-anchored.md`; (b) the free-search candidates in `results-optimise.md`, which separate better but are uneven in weight; (c) Okabe–Ito on dark plus a hand-made light set, held to the same bars.
   - Recommendation: (a). Look at swatches on both grounds before choosing.
2. **Which sequential scale is the default?**
   - Options: (a) cividis, which passes REQ-THEME-080 with 7.0; (b) viridis, the most familiar, which fails it at 4.4 under protanopia; (c) magma (6.2).
   - Recommendation: (a), with viridis and magma available by name.
3. **What colours are up and down?**
   - Options: (a) green and red, the US convention, weak under deuteranopia (9.3 for unicel's pair); (b) blue and orange (at least 50.5 under every vision); (c) by locale.
   - Recommendation: (b) as the default, with fill always differing and the tokens overridable.
4. **Does a time axis run right to left in RTL locales?**
   - Options: (a) never; (b) always; (c) an option, off by default.
   - Recommendation: (c).
5. **Patterns in colour mode.**
   - Options: (a) one-ink cues only in one ink; (b) markers and dashes always; (c) a `patterns` option, `auto` meaning on in one ink and off in colour.
   - Recommendation: (c), since direct labels and the legend already satisfy 1.4.1.
6. **Is the visible control row (zoom, reset, step, pause) on by default?**
   - It is how 2.5.7 and 2.2.2 are met without the keyboard.
   - Options: (a) always; (b) on focus and hover; (c) only with `controls`.
   - Recommendation: (b).

## Sources

- WCAG 2.2: https://www.w3.org/TR/WCAG22/ (relative luminance, contrast ratio); Understanding pages https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html (1.4.11), https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html (1.4.1), https://www.w3.org/WAI/WCAG22/Understanding/content-on-hover-or-focus.html (1.4.13), https://www.w3.org/WAI/WCAG22/Understanding/pause-stop-hide.html (2.2.2), https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html (2.5.7), https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html (4.1.3), https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html (2.4.7).
- WAI-ARIA APG grid pattern: https://www.w3.org/WAI/ARIA/apg/patterns/grid/; slider pattern: https://www.w3.org/WAI/ARIA/apg/patterns/slider/.
- Graphics-ARIA 1.0 (W3C Recommendation, 2018-10-02): https://www.w3.org/TR/graphics-aria-1.0/.
- MDN, `forced-colors`: https://developer.mozilla.org/en-US/docs/Web/CSS/@media/forced-colors.
- Cross-root ARIA: Nolan Lawson, "Shadow DOM and accessibility: the trouble with ARIA" (2022), https://nolanlawson.com/2022/11/28/shadow-dom-and-accessibility-the-trouble-with-aria/; blink-dev "Intent to Ship: Reference Target", https://groups.google.com/a/chromium.org/g/Blink-dev/c/eBx3sXIfJnw.
- Chartability: https://chartability.fizz.studio/ and the POUR-CAF workbook https://chartability.github.io/POUR-CAF/.
- Highcharts accessibility module: https://www.highcharts.com/docs/accessibility/accessibility-module; `accessibility.announceNewData`: https://api.highcharts.com/highcharts/accessibility.announceNewData.
- Olli: https://github.com/umwelt-data/olli.
- Machado, Oliveira, Fernandes, "A Physiologically-based Model for Simulation of Color Vision Deficiency", IEEE TVCG 2009: https://www.inf.ufrgs.br/~oliveira/pubs_files/CVD_Simulation/CVD_Simulation.html; DaltonLens's review of simulation methods: https://daltonlens.org/opensource-cvd-simulation/.
- Okabe and Ito, "Color Universal Design": https://jfly.uni-koeln.de/color/; Paul Tol's schemes: https://personal.sron.nl/~pault/; d3-scale-chromatic 3.1.0 sources (Tableau10, observable10, Dark2, Set2, RdBu, PuOr, BrBG): https://unpkg.com/d3-scale-chromatic@3.1.0/src/; cividis: Nuñez, Anderton, Renslow, PLOS ONE 2018, https://doi.org/10.1371/journal.pone.0199239; viridis and magma as shipped in matplotlib 3.10.0.
- Unicode: UAX #9 https://www.unicode.org/reports/tr9/, UAX #11 https://www.unicode.org/reports/tr11/, UAX #29 https://www.unicode.org/reports/tr29/, UAX #15 https://www.unicode.org/reports/tr15/; CLDR 48 through Node 26.5.1's ICU 78.3.
- ICU4X: https://github.com/unicode-org/icu4x (`icu_decimal` 2.3.0, `icu_datetime` 2.x as resolved in `discovery/bench/a11y-theming/Cargo.lock`).
- unicel: `spec/SPEC.md` (§3.1 REQ-GRID-001, REQ-GRID-060; §3.2 REQ-CHAR-020; §3.4; §4 REQ-PIECE-508, REQ-MONO-006; §7 REQ-EL-050, REQ-MONO-001, REQ-MONO-004; §8 REQ-MOUNT-010, -020, -070, -090, -120; §10.6; §10.10), `spec/ROADMAP.md` §7, `spec/adr/016-two-colours-a-cell.md`, `026-single-palettes.md`, `118-mono-font-robustness.md`, `119-deterministic-halftone.md`, `121-mono-small-sizes-on-canvas.md`, `crates/unicel/src/palette.rs`, `crates/unicel/src/mono.rs:21-66`, `crates/unicel/src/pixels.rs:186-188`, `web/src/element.ts:210-216`, `web/src/mount.ts:78-101, 435-457`, `site/e2e/a11y.spec.ts`, `site/src/site.css:6-40`, `crates/unicel/src/pieces/data/candlesticks.rs:26-28`.
- Bench: `discovery/bench/a11y-theming/` (README, scripts, `results-*.md`).
