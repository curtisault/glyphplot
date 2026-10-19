# a11y-theming bench (throwaway)

Measurements for `discovery/a11y-theming.md`. Nothing here is part of glyphplot;
delete it when the plan no longer needs it. Run everything from the repository root.

| File | Measures | Rerun |
|---|---|---|
| `palettes.py` | WCAG contrast of palettes against the grounds; CIEDE2000 between colours under normal vision and Machado 2009 dichromacies; sequential and diverging scales | `mise exec -- uv run --no-project --with colour-science==0.4.6 --with numpy==2.5.4 --with matplotlib==3.10.0 python -W ignore discovery/bench/a11y-theming/palettes.py` → `results-palettes.md` |
| `optimise.py` | A free search for 8- and 10-colour palettes that keep 3:1 and maximise the smallest ΔE00 under all four visions | same command, `optimise.py` → `results-optimise.md` (about 2.5 min) |
| `anchored.py` | The same search held to Okabe–Ito's hues and a lightness band | same command, `anchored.py` → `results-anchored.md` (about 1.5 min) |
| `updown.py` | Candle up/down colour pairs | same command, `updown.py` → `results-updown.md` |
| `intl.mjs` + `src/bin/widths.rs` | What `Intl` prints for tick labels by locale, and the cell widths (unicode-width 0.2.2) and grapheme counts (unicode-segmentation 1.13.2) of those and other labels | `mise exec -- cargo build --release --manifest-path discovery/bench/a11y-theming/Cargo.toml --bin widths && mise exec -- node discovery/bench/a11y-theming/intl.mjs \| discovery/bench/a11y-theming/target/release/widths` → `results-widths.md` |
| `sizes.sh` + `src/lib.rs` | wasm size of number formatting by hand, with ICU4X `icu_decimal` 2.3, and with ICU4X `icu_datetime` 2.x (compiled data, every locale) | `cd discovery/bench/a11y-theming && mise exec -- bash sizes.sh` |
| `localepack.mjs` | A locale pack read from `Intl.formatToParts` (bytes, build time) and the cost of formatting 20 tick labels with `Intl.NumberFormat` | `mise exec -- node discovery/bench/a11y-theming/localepack.mjs` → `results-localepack.md` |
| `table.js` + `table-run.mjs` | Build-and-layout time of a visually hidden `<table>` of N rows in headless Chromium (any playwright-core install under `<dir>/node_modules`) | `ROWS="100, 250, 500, 1000, 2000, 5000, 10000" BROWSERS=chromium NODE_PATH=<dir>/node_modules mise exec -- node discovery/bench/a11y-theming/table-run.mjs` → `results-table.md`. 100k rows did not finish within 5 minutes; that playwright-core's WebKit build is not installed. |

The Cargo package has an empty `[workspace]` table so it builds on its own; its
`target/` is ignored by the repository's `.gitignore`.
