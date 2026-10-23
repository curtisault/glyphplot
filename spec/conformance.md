# glyphplot — Conformance

Every requirement, built ([SPEC.md](SPEC.md)) or not yet
([ROADMAP.md](ROADMAP.md)), mapped to the test or check that covers it or
will. v0.1.0 to v0.5.0 are built and their checks run; the checks of later milestones are
planned, their Status `—`. The planned names follow
[discovery/testing.md](../discovery/testing.md) where it gives one; the rest
are proposals in the same style.

When a test lands, set its requirement's Status to `yes` (the check covers
the whole requirement) or `partial` (it covers part, and the Planned check
column says which part), keep the planned check name or correct it to the
real one, and update the counts in the [summary](#summary), all in the same
commit as the test. A requirement whose only check is manual or review
stays `—`.

## Kinds of check

- **Rust test**: `file::test_name`, run by `mise run test:rust`. `contract` is
  `crates/glyphplot/tests/contract.rs`, `budgets` is
  `crates/glyphplot/tests/budgets.rs` and `reference` is
  `crates/glyphplot/tests/reference.rs`; "rule N" is a row of the plan's
  contract rules table. A unit test is written `<file> tests::<name>`, for
  example `crates/glyphplot/src/scale.rs tests::invert_undoes_map`.
  `crates/cli/tests/terminal.rs` runs in a pseudo-terminal (Unix only).
- **Node test**: `web/test/<file>.test.mjs`, run by `mise run test:wasm`
  against the release modules and again against a debug build
  (REQ-TEST-020).
- **Browser test**: `site/e2e/<file>.spec.ts`, run by `mise run test:site`
  (Playwright, headless Chromium, the release build; `--smoke` in `check`,
  all of it in `signoff`).
- **Task**: `mise run <task>`, the task itself being the check (`size`,
  `bench:web`, `datasets:check`, …).
- **Lint**: a step of `mise run lint`, named in parentheses.
- **Manual**: the owner's check on real devices and browsers (REQ-TEST-030).
- **Review**: read in review; no automated check.

Several checks may be listed for one requirement, separated by commas.

## 1. Architecture and build

### 1.1 Layers (ARCH)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-ARCH-010 | v0.1.0 | `mise run lint` (`scripts/deps.sh`: `cargo tree -e normal -p glyphplot` against `serde`, `serde_json`, `unicode-width`) | yes |
| REQ-ARCH-020 | v0.3.0 | `web/test/player.test.mjs`, review (`crates/wasm` and `crates/cli` define no chart) | partial |
| REQ-ARCH-030 | v0.3.0 | `mise run lint` (no import of `web/pkg` outside `web/src`), review (the site imports only the shell's exports) | — |
| REQ-ARCH-040 | v0.1.0 | `mise run lint` (`cargo check` of each family feature alone), `mise run build:wasm` | yes |
| REQ-ARCH-050 | v0.3.0 | `mise run lint` (`cargo check -p glyphplot --no-default-features --features <family>` per family) | yes |
| REQ-ARCH-060 | v0.1.0 | `mise run check`, review (`Cargo.toml`, `rust-toolchain.toml`, `mise.toml`) | partial |
| REQ-ARCH-070 | v0.1.0 | `mise run lint` (`scripts/deps.sh`: `cargo metadata`, no git source) | yes |
| REQ-ARCH-080 | v0.8.0 | `crates/cli/tests/cli.rs::list_names_every_type_and_pack` | — |

### 1.2 Independence (INDEP)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-INDEP-010 | v0.1.0 | `mise run lint` (`scripts/deps.sh`: no manifest, task or script names `animate-unicode` or a `unicel` crate or package), review (no file copied, ported or generated from unicel) | partial |
| REQ-INDEP-020 | v0.1.0 | `mise run lint` (`scripts/deps.sh`; a test edit adding `../animate-unicode` as a path dependency fails it, checked 2026-10-18) | yes |

### 1.3 Tasks (BUILD)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-BUILD-010 | v0.1.0 | `mise run check` | yes |
| REQ-BUILD-020 | v0.1.0 | `mise run check`, review (the task graph in `mise.toml`) | yes |
| REQ-BUILD-030 | v0.1.0 | `mise run build:wasm`, `mise run size` (reads `budgets.json` and `sizes.json`), `web/test/player.test.mjs` (v0.3.0) | yes |
| REQ-BUILD-040 | v0.3.0 | `mise run build:wasm:debug`, review (the staged swap) | partial |
| REQ-BUILD-050 | v0.1.0 | review (`[profile.release]` and the wasm-opt flags), `mise run size` | partial |
| REQ-BUILD-060 | v0.1.0 | `mise run lint` (`scripts/generated.sh`: `git ls-files` holds no generated output) | yes |
| REQ-BUILD-070 | v0.2.0 | `mise run test:rust`, review (`[profile.contract]`) | yes |
| REQ-BUILD-080 | v0.3.0 | manual (`mise run dev` rebuilds and reloads) | — |
| REQ-BUILD-090 | v0.4.0 | `mise run signoff` (logs the time of `check`), manual | yes |

### 1.4 Lint (LINT)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-LINT-010 | v0.1.0 | `mise run lint` (itself) | yes |
| REQ-LINT-020 | v0.3.0 | `mise run lint` (`tsc --noEmit`, `elm-format --validate`) | yes |
| REQ-LINT-030 | v0.1.0 | `mise run lint` (rustfmt, `missing_docs`, rustdoc `-D warnings`) | yes |
| REQ-LINT-040 | v0.2.0 | `mise run lint` (clippy `disallowed-methods`) | yes |

### 1.5 Local CI (CI)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CI-010 | v0.1.0 | review (no `.github/workflows`) | — |
| REQ-CI-020 | v0.4.0 | `mise run signoff` (itself), its run on #7 (2026-10-20: npm ci, check with all of test:site, test:budgets, bench:web and bench:load passed; a success status posted) | yes |

## 2. Core: grid, glyphs, colour

### 2.1 Grid (GRID)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-GRID-010 | v0.2.0 | `crates/glyphplot/src/grid.rs tests::cells_are_a_code_point_and_two_indices`, `crates/glyphplot/src/grid.rs tests::constants_are_fixed` | yes |
| REQ-GRID-020 | v0.2.0 | `crates/glyphplot/src/grid.rs tests::resize_blanks_and_reuses`, `crates/glyphplot/src/grid.rs tests::resize_to_zero_panics` | yes |
| REQ-GRID-030 | v0.2.0 | `crates/glyphplot/src/grid.rs tests::new_and_clear_blank_every_cell`, `crates/glyphplot/src/grid.rs tests::fill_keeps_backs` | yes |
| REQ-GRID-040 | v0.2.0 | `crates/glyphplot/src/grid.rs tests::draws_clip_at_the_edges`, `crates/glyphplot/src/grid.rs tests::lines_include_both_ends` | yes |
| REQ-GRID-050 | v0.2.0 | `crates/glyphplot/src/grid.rs tests::set_wide_writes_its_tail`, `crates/glyphplot/src/grid.rs tests::set_wide_at_the_edge_draws_nothing` | partial |
| REQ-GRID-060 | v0.2.0 | `crates/glyphplot/src/grid.rs tests::text_joins_the_rows`, `crates/glyphplot/src/grid.rs tests::text_skips_tails_and_replaces_invalid` | yes |
| REQ-GRID-070 | v0.2.0 | `crates/glyphplot/src/grid.rs tests::slices_are_row_major`, `crates/glyphplot/src/grid.rs tests::getters_panic_off_the_grid` | yes |

### 2.2 Sub-cell canvases and cell mapping (CANVAS)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CANVAS-010 | v0.2.0 | `crates/glyphplot/src/canvas.rs tests::lines_and_circles_clip`, `crates/glyphplot/src/canvas.rs tests::drawing_sets_only_cells_with_dots` | yes |
| REQ-CANVAS-020 | v0.2.0 | `crates/glyphplot/src/cells.rs tests::column_and_x_agree`, `contract::hits_name_what_was_drawn` (rule 19) | yes |
| REQ-CANVAS-030 | v0.2.0 | `crates/glyphplot/src/canvas.rs tests::glyphs_match_their_unicode_names`, `crates/glyphplot/src/canvas.rs tests::an_empty_cell_is_a_space`, `mise run lint` (`scripts/octants.py` regenerates the table byte for byte) | yes |
| REQ-CANVAS-040 | v0.3.0 | `crates/glyphplot/src/canvas.rs tests::a_cell_takes_the_highest_series`, `contract::every_chart_keeps_the_contract` (rule 11) | yes |

### 2.3 Charsets (CHAR)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CHAR-010 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 5) | yes |
| REQ-CHAR-020 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 5, `labels-wide`), `crates/glyphplot/src/grid.rs tests::set_wide_writes_its_tail` | yes |

### 2.4 Colour (COLOR)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-COLOR-010 | v0.2.0 | `contract::every_chart_keeps_the_contract` (rule 6) | yes |
| REQ-COLOR-020 | v0.2.0 | `crates/glyphplot/src/theme.rs tests::palette_layout_is_fixed`, `contract::every_chart_keeps_the_contract` (rule 6) | yes |
| REQ-COLOR-030 | v0.2.0 | `crates/glyphplot/src/theme.rs tests::resolve_gives_255_entries`, `crates/glyphplot/src/theme.rs tests::theme_change_draws_no_frame` | partial |
| REQ-COLOR-040 | v0.2.0 | `crates/glyphplot/src/theme.rs tests::scale_ramp_steps_within_delta_e_2_5` | yes |
| REQ-COLOR-050 | v0.8.0 | `crates/cli/src/color.rs tests::palette_maps_once_to_256_and_16` | — |

## 3. Data

### 3.1 The data contract (DATA)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-DATA-010 | v0.2.0 | `web/test/schemas.test.mjs`, `crates/glyphplot/src/data.rs tests::reads_every_column_type` | yes |
| REQ-DATA-020 | v0.2.0 | `crates/glyphplot/src/data.rs tests::ns_timestamps_round_trip` | yes |
| REQ-DATA-030 | v0.2.0 | `crates/glyphplot/src/data.rs tests::every_missing_form_reads_as_missing`, `contract::every_chart_keeps_the_contract` (rule 23) | yes |
| REQ-DATA-040 | v0.2.0 | `contract::every_chart_keeps_the_contract` (rule 26) | yes |
| REQ-DATA-050 | v0.3.0 | `web/test/data.wasm.test.mjs` (a typed column reaches wasm memory with one copy), `web/test/data.test.mjs`, `mise run size --charts` (the no-parser symbol check, planned) | partial |
| REQ-DATA-060 | v0.3.0 | `web/test/data.wasm.test.mjs` (memory.buffer read after the address; a 1,000,000-row fill across a growth) | yes |
| REQ-DATA-070 | v0.3.0 | `web/test/data.wasm.test.mjs` (replace at most 10 ms, median of 5), `web/test/memory.test.mjs` | yes |
| REQ-DATA-080 | v0.2.0 | `crates/glyphplot/src/data.rs tests::channels_resolve_by_encode_role_then_name` | partial |
| REQ-DATA-090 | v0.2.0 | `crates/glyphplot/tests/zones.rs::frames_equal_under_utc_and_tokyo` | partial |
| REQ-DATA-100 | v0.5.0 | `crates/glyphplot/src/format.rs tests::unit_scale_and_offset_change_labels_only`, `crates/glyphplot/src/charts/line/tests.rs tests::display_transform_applies_to_ticks_readout_and_table`, `crates/glyphplot/src/charts/line/spark/tests.rs tests::text_and_readout_take_the_display_transform` (scale and offset on the line chart's ticks, readout and table and the sparkline's text and readout, the unit on readouts and table cells; no axis writes the unit yet) | partial |
| REQ-DATA-110 | v0.2.0 | `contract::every_chart_keeps_the_contract` (rule 28) | yes |
| REQ-DATA-120 | v0.3.0 | `web/test/data.test.mjs`, `site/e2e/element.spec.ts` | yes |
| REQ-DATA-130 | v0.2.0 | `contract::replace_is_whole` (rule 15) | yes |
| REQ-DATA-140 | v0.6.0 | `crates/glyphplot/src/arrow.rs tests::reads_every_column_type`, `web/test/data.test.mjs`, `mise run size --charts` | — |
| REQ-DATA-150 | v0.6.0 | `site/e2e/streaming.spec.ts` (CSV, JSON, NDJSON served by the site; Arrow drawn through `set_arrow` fixme until the module has it), `web/test/stream.test.mjs`, `web/test/stream.wasm.test.mjs` (Arrow through `set_arrow` todo until the module has it) | partial |
| REQ-DATA-160 | v0.9.0 | `crates/glyphplot/src/data.rs tests::graph_edges_resolve_and_drop_unknowns` | — |
| REQ-DATA-170 | v0.12.0 | `crates/glyphplot/src/data.rs tests::julian_epoch_offsets`, `contract::every_chart_matches_its_snapshots` (TESS fixture) | — |

### 3.2 Streaming (STREAM)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-STREAM-010 | v0.6.0 | `web/test/stream.wasm.test.mjs`, `web/test/stream.test.mjs`, `site/e2e/streaming.spec.ts` | yes |
| REQ-STREAM-020 | v0.6.0 | `web/test/memory.test.mjs` | — |
| REQ-STREAM-030 | v0.6.0 | `crates/glyphplot/src/ring.rs tests::retain_age_keeps_the_newest_span` | — |
| REQ-STREAM-040 | v0.6.0 | `crates/glyphplot/src/ring.rs tests::equal_x_replaces_and_less_is_late`, `web/test/data.test.mjs` | — |
| REQ-STREAM-050 | v0.6.0 | `web/test/stream.wasm.test.mjs` (todo until the module has `staging` and `append`) | — |
| REQ-STREAM-060 | v0.3.0 | `contract::replace_is_whole` (rule 15) | yes |
| REQ-STREAM-070 | v0.6.0 | `web/test/stream.wasm.test.mjs`, `site/e2e/streaming.spec.ts` | yes |

### 3.3 Downsampling (SAMPLE)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-SAMPLE-010 | v0.3.0 | `contract::downsampling_is_exact` (rule 27) | yes |
| REQ-SAMPLE-020 | v0.3.0 | `crates/glyphplot/src/downsample.rs tests::unchanged_view_reads_no_data`, `crates/glyphplot/src/charts/line/tests.rs tests::unchanged_frames_read_no_data_and_draw_no_base` | yes |
| REQ-SAMPLE-030 | v0.3.0 | `web/test/data.wasm.test.mjs` (the M4 frame, an upper bound) | yes |
| REQ-SAMPLE-040 | v0.5.0 | `crates/glyphplot/src/charts/candles/resample.rs tests::picks_the_smallest_fitting_interval`, `contract::every_chart_matches_its_snapshots` | yes |
| REQ-SAMPLE-050 | v0.9.0 | `crates/glyphplot/src/charts/scatter/mod.rs tests::every_visible_point_sets_a_dot` | — |
| REQ-SAMPLE-060 | v0.5.0 | `crates/glyphplot/src/downsample.rs tests::lttb_keeps_ends_and_order`, `reference::lttb_matches_the_reference` | yes |

## 4. Scales, ticks, formatting and layout

### 4.1 Scales (SCALE)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-SCALE-010 | v0.2.0 | `crates/glyphplot/src/scale.rs tests::invert_undoes_map` | yes |
| REQ-SCALE-020 | v0.2.0 | `crates/glyphplot/src/scale.rs tests::domain_forms_are_pure`, `contract::every_chart_keeps_the_contract` (rule 8) | yes |
| REQ-SCALE-030 | v0.2.0 | `reference::nice_matches_d3_scale` | yes |
| REQ-SCALE-040 | v0.5.0 | `crates/glyphplot/src/scale.rs tests::log_inverts_for_any_base`, `crates/glyphplot/src/scale.rs tests::band_cells_are_equal` | yes |
| REQ-SCALE-050 | v0.5.0 | `crates/glyphplot/src/scale.rs tests::symlog_ticks_are_zero_and_powers_of_ten` | yes |
| REQ-SCALE-060 | v0.5.0 | `crates/glyphplot/src/scale.rs tests::index_time_skips_gaps`, `contract::every_chart_matches_its_snapshots` (`ohlcv-days`) | yes |
| REQ-SCALE-070 | v0.9.0 | `crates/glyphplot/src/scale.rs tests::colour_scales_map_to_palette_indices` | — |
| REQ-SCALE-080 | v0.12.0 | `crates/glyphplot/src/scale.rs tests::angle_scale_wraps` | — |

### 4.2 Ticks (TICK)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-TICK-010 | v0.2.0 | `reference::ticks_match_d3_array`, `web/test/reference.test.mjs` | yes |
| REQ-TICK-020 | v0.2.0 | `reference::time_ticks_match_d3_time` | yes |
| REQ-TICK-030 | v0.2.0 | `crates/glyphplot/src/ticks.rs tests::ticks_are_ordered_inside_and_about_count`, `crates/glyphplot/src/layout.rs tests::labels_never_share_a_cell` | yes |
| REQ-TICK-040 | v0.2.0 | `crates/glyphplot/src/ticks.rs tests::marks_take_the_nearest_cell_and_dot` | yes |
| REQ-TICK-050 | v0.5.0 | `crates/glyphplot/src/ticks.rs tests::snap_rows_puts_every_tick_on_a_row` | partial |
| REQ-TICK-060 | v0.5.0 | `crates/glyphplot/src/ticks.rs tests::log_axes_label_only_powers`, `crates/glyphplot/src/layout.rs tests::log_marks_are_placed_by_the_transform` | yes |
| REQ-TICK-070 | v0.5.0 | `crates/glyphplot/src/ticks.rs tests::context_row_marks_day_boundaries`, `crates/glyphplot/src/layout.rs tests::time_axes_under_two_days_have_a_context_row`, `crates/glyphplot/src/charts/line/tests.rs tests::time_ticks_and_labels_follow_the_zone` | yes |

### 4.3 Formatting (FMT)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-FMT-010 | v0.2.0 | `reference::format_matches_d3_format` | yes |
| REQ-FMT-020 | v0.2.0 | `crates/glyphplot/src/format.rs tests::rejects_unknown_specifiers`, `reference::format_matches_d3_format` | yes |
| REQ-FMT-030 | v0.2.0 | `contract::every_chart_keeps_the_contract` (rule 22), `crates/glyphplot/src/format.rs tests::formatted_numbers_parse_back` | yes |
| REQ-FMT-040 | v0.5.0 | `crates/glyphplot/src/format.rs tests::unit_table` | yes |
| REQ-FMT-050 | v0.5.0 | `crates/glyphplot/src/format.rs tests::strftime_subset`, `crates/glyphplot/src/format.rs tests::hour_cycle_follows_the_locale` | yes |
| REQ-FMT-060 | v0.3.0 | `crates/glyphplot/src/format.rs tests::minus_and_group_separators`, `web/test/locale.test.mjs` | yes |
| REQ-FMT-070 | v0.12.0 | `crates/glyphplot/src/format.rs tests::right_ascension_and_declination` | — |

### 4.4 Time zones and locales (TZ, I18N)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-TZ-010 | v0.2.0 | `crates/glyphplot/src/time.rs tests::default_zone_is_utc`, `mise run lint` (clippy `disallowed-methods`: no clock) | yes |
| REQ-TZ-020 | v0.5.0 | `web/test/locale.test.mjs`, `site/e2e/element.spec.ts` | yes |
| REQ-TZ-030 | v0.5.0 | `reference::local_ticks_match_d3_scale_time`, `crates/glyphplot/src/layout.rs tests::built_scales_tick_in_their_zone`, `crates/glyphplot/src/charts/line/tests.rs tests::time_ticks_and_labels_follow_the_zone`, `web/test/zone.wasm.test.mjs`, `web/test/reference.test.mjs` (`zone-ticks.txt` in wasm) | yes |
| REQ-TZ-040 | v0.8.0 | `crates/cli/tests/cli.rs::frames_match_the_element_in_a_zone` | — |
| REQ-TZ-050 | v0.5.0 | `site/e2e/element.spec.ts` (the page's zone passed in), `crates/glyphplot/src/time.rs tests::default_zone_is_utc` | yes |
| REQ-I18N-010 | v0.3.0 | `web/test/locale.test.mjs` | yes |
| REQ-I18N-020 | v0.3.0 | `web/test/locale.test.mjs`, `mise run lint` (`scripts/deps.sh`: the REQ-SIZE-060 crate ban) | partial |
| REQ-I18N-030 | v0.2.0 | `crates/glyphplot/src/format.rs tests::default_locale_is_plain` | yes |
| REQ-I18N-040 | v0.2.0 | `crates/glyphplot/src/layout.rs tests::labels_count_unicode_width`, `contract::every_chart_keeps_the_contract` (rule 5, `labels-wide`) | yes |
| REQ-I18N-050 | v0.3.0 | `web/test/data.test.mjs` (NFC), `crates/glyphplot/src/text.rs tests::widths_count_cells_and_drop_joined_scalars`, `crates/glyphplot/src/grid.rs tests::set_wide_writes_its_tail` | partial |
| REQ-I18N-060 | v0.9.0 | `crates/glyphplot/src/text.rs tests::rtl_runs_reorder_by_uax9`, `contract::every_chart_matches_its_snapshots` (RTL locale) | — |
| REQ-I18N-070 | v0.8.0 | `crates/cli/tests/cli.rs::locale_file_is_read` | — |

### 4.5 Layout and text (LAYOUT, TEXT, AXIS, ANNOT)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-LAYOUT-010 | v0.2.0 | `crates/glyphplot/src/layout.rs tests::layout_is_pure_and_ordered`, `contract::every_chart_keeps_the_contract` (rule 8) | yes |
| REQ-LAYOUT-020 | v0.2.0 | `crates/glyphplot/src/layout.rs tests::small_sizes_drop_axes_title_and_labels` | yes |
| REQ-LAYOUT-030 | v0.2.0 | `crates/glyphplot/src/layout.rs tests::wide_gutters_drop_the_right_axis_first` | yes |
| REQ-LAYOUT-040 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 4), `contract::every_size_draws_without_panic` | partial |
| REQ-LAYOUT-050 | v0.5.0 | `crates/glyphplot/src/layout.rs tests::panes_split_by_weight` | yes |
| REQ-LAYOUT-060 | v0.5.0 | `crates/glyphplot/src/layout.rs tests::auto_legend_placement` | yes |
| REQ-LAYOUT-070 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 1), `site/e2e/charts.spec.ts` | partial |
| REQ-TEXT-010 | v0.2.0 | `crates/glyphplot/src/layout.rs tests::labels_never_share_a_cell`, `contract::every_chart_keeps_the_contract` (rule 24) | yes |
| REQ-TEXT-020 | v0.2.0 | `crates/glyphplot/src/layout.rs tests::draw_order_is_z_order`, `contract::every_chart_matches_its_snapshots` | yes |
| REQ-AXIS-010 | v0.5.0 | `crates/glyphplot/src/layout.rs tests::left_and_right_axes_bind_their_scales`, `contract::every_chart_matches_its_snapshots` | partial |
| REQ-ANNOT-010 | v0.5.0 | `crates/glyphplot/src/annotate.rs tests::annotations_clip_to_their_pane`, `crates/glyphplot/src/annotate.rs tests::axis_label_hides_overlapped_ticks` | yes |

## 5. View, input and interaction

### 5.1 View state (VIEW)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-VIEW-010 | v0.3.0 | `contract::input_is_deterministic` (rule 17), `contract::every_chart_keeps_the_contract` (rule 8) | yes |
| REQ-VIEW-020 | v0.3.0 | `contract::view_round_trips` (rule 16), `crates/glyphplot/src/view.rs tests::null_means_fitted` | yes |
| REQ-VIEW-030 | v0.3.0 | `web/test/view.test.mjs`, `site/e2e/element.spec.ts` | yes |
| REQ-VIEW-040 | v0.3.0 | `site/e2e/element.spec.ts` | yes |
| REQ-VIEW-050 | v0.3.0 | `crates/glyphplot/src/view.rs tests::motion_frames_depend_only_on_t`, `site/e2e/a11y.spec.ts` (reduced motion) | yes |
| REQ-VIEW-060 | v0.6.0 | `crates/glyphplot/src/view.rs tests::follow_keeps_the_span` (the core's follow on append), `web/test/stream.wasm.test.mjs` (End, the round trip, the shell's fallback; the module's follow todo until it has `append`), `site/e2e/streaming.spec.ts` | partial |

### 5.2 Input (INPUT)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-INPUT-010 | v0.3.0 | `contract::input_is_deterministic` (rule 17) | partial |
| REQ-INPUT-020 | v0.3.0 | `web/test/input.test.mjs`, `web/test/events.wasm.test.mjs` | partial |
| REQ-INPUT-030 | v0.3.0 | `web/test/input.test.mjs` | yes |
| REQ-INPUT-040 | v0.3.0 | `site/e2e/interaction.spec.ts`, `crates/glyphplot/src/charts/line/tests.rs tests::drags_pan_brush_and_zoom`, `tests::wheel_zooms_with_ctrl_and_ends_final` | partial |

### 5.3 Hit testing and the overlay (HIT, OVERLAY)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-HIT-010 | v0.3.0 | `contract::hits_name_what_was_drawn` (rules 19, 20), `crates/glyphplot/src/hit.rs tests::snap_matches_brute_force` | yes |
| REQ-HIT-020 | v0.3.0 | `crates/glyphplot/src/hit.rs tests::shared_column_carries_its_aggregate`, `crates/glyphplot/src/charts/line/tests.rs tests::readout_names_the_nearest_x` | yes |
| REQ-HIT-030 | v0.9.0 | `budgets::every_chart_keeps_its_time_budgets` (scatter hover), `crates/glyphplot/src/charts/scatter/index.rs tests::scan_answers_before_the_index` | — |
| REQ-OVERLAY-010 | v0.3.0 | `budgets::every_chart_keeps_its_time_budgets` (pointer move), `crates/glyphplot/src/overlay.rs tests::hover_does_not_redraw_the_base` | yes |
| REQ-OVERLAY-020 | v0.3.0 | `crates/glyphplot/src/overlay.rs tests::readout_stays_and_hides_on_escape`, `site/e2e/interaction.spec.ts` | partial |

### 5.4 Events out and linking (EVENT, LINK)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-EVENT-010 | v0.3.0 | `web/test/events.test.mjs`, `site/e2e/interaction.spec.ts`, `crates/glyphplot/src/charts/line/tests.rs tests::legend_clicks_toggle` (`plot-legend` once a click or a tap) | partial |
| REQ-EVENT-020 | v0.3.0 | `site/e2e/interaction.spec.ts`, `web/test/events.test.mjs` | yes |
| REQ-EVENT-030 | v0.3.0 | `site/e2e/interaction.spec.ts` | yes |
| REQ-LINK-010 | v0.5.0 | `site/e2e/element.spec.ts` | yes |

### 5.5 Keyboard and touch (KEY, TOUCH)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-KEY-010 | v0.3.0 | `site/e2e/interaction.spec.ts`, `contract::input_is_deterministic` (rule 17) | partial |
| REQ-TOUCH-010 | v0.3.0 | `site/e2e/interaction.spec.ts` (CDP touch), manual (phones), `crates/glyphplot/src/charts/line/tests.rs tests::touch_gestures`, `web/test/input.test.mjs` | partial |
| REQ-TOUCH-020 | v0.3.0 | `site/e2e/interaction.spec.ts`, review (no touch listeners in `web/src`) | — |

## 6. The chart contract and the registry

### 6.1 Charts, meta and options (CHART, OPT, REG)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CHART-010 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 1), `mise run lint` (per-family `cargo check`), review (family folders) | partial |
| REQ-CHART-020 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 1), `web/test/schemas.test.mjs` | yes |
| REQ-CHART-030 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 2) | yes |
| REQ-CHART-040 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 3), review (the `Chart` trait) | partial |
| REQ-CHART-050 | v0.3.0 | review (`Env`) | — |
| REQ-CHART-060 | v0.3.0 | `crates/glyphplot/src/charts/line/tests.rs tests::later_series_wins_a_shared_cell` | partial |
| REQ-OPT-010 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 2), `crates/glyphplot/src/options.rs tests::getters_fall_back`, `crates/glyphplot/src/options.rs tests::only_an_object_or_nothing_parses`, `crates/glyphplot/src/registry.rs tests::options_record_what_the_chart_reads` | yes |
| REQ-OPT-020 | v0.3.0 | `crates/glyphplot/src/registry.rs tests::set_options_keeps_data_and_view`, `site/e2e/element.spec.ts` | yes |
| REQ-REG-010 | v0.3.0 | `crates/glyphplot/src/registry.rs tests::make_errors_name_the_problem`, `web/test/player.test.mjs` | yes |
| REQ-REG-020 | v0.3.0 | `crates/glyphplot/src/registry.rs tests::no_reserved_names` | yes |

### 6.2 The contract test (CONTRACT, FIX)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CONTRACT-010 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rules 1–9), `contract::every_chart_matches_its_snapshots` (rule 29), `contract::selftest_rejects_broken_charts` | yes |
| REQ-CONTRACT-020 | v0.3.0 | `contract::every_chart_keeps_the_contract` (itself), review (`CHART=` and the table) | partial |
| REQ-CONTRACT-030 | v0.4.0 | `contract::every_chart_keeps_the_contract` (rules 10–13, 21–26, 28), `contract::replace_is_whole`, `contract::view_round_trips`, `contract::input_is_deterministic`, `contract::hits_name_what_was_drawn`, `contract::downsampling_is_exact`, `web/test/events.test.mjs` (rule 18) | partial |
| REQ-CONTRACT-040 | v0.6.0 | `contract::streaming_equals_replace` (rule 14) | — |
| REQ-CONTRACT-050 | v0.4.0 | `budgets::every_chart_keeps_its_time_budgets` (rule 30) | yes |
| REQ-CONTRACT-060 | v0.4.0 | `contract::selftest_rejects_broken_charts` | yes |
| REQ-FIX-010 | v0.2.0 | `crates/glyphplot/src/fixtures.rs tests::generations_are_bitwise_equal`, `web/test/reference.test.mjs` (fixtures in wasm) | yes |
| REQ-FIX-020 | v0.5.0 | `crates/glyphplot/tests/data_readme.rs::every_dataset_is_small_and_listed` | yes |

### 6.3 Snapshots, references and properties (SNAP, REF, PROP)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-SNAP-010 | v0.3.0 | `contract::every_chart_matches_its_snapshots` (rule 29) | yes |
| REQ-SNAP-020 | v0.3.0 | `contract::every_chart_matches_its_snapshots`, `crates/glyphplot/tests/common/snap.rs` (the writer), `crates/glyphplot/tests/common/snap.rs tests::runs_read_as_the_spec_writes_them` | yes |
| REQ-SNAP-030 | v0.3.0 | `mise run snap`, `contract::every_chart_matches_its_snapshots`, `web/test/snapshot.test.mjs` | yes |
| REQ-SNAP-040 | v0.3.0 | review (commit messages) | — |
| REQ-REF-010 | v0.2.0 | `reference::ticks_match_d3_array`, `reference::nice_matches_d3_scale`, `reference::time_ticks_match_d3_time`, `reference::format_matches_d3_format`, `reference::log_ticks_match_d3_scale`, `reference::local_ticks_match_d3_scale_time`, `reference::lttb_matches_the_reference`, `web/test/reference.test.mjs` (every capture file in the debug build), `mise run fixtures` | yes |
| REQ-PROP-010 | v0.2.0 | `crates/glyphplot/src/scale.rs tests::invert_undoes_map`, `crates/glyphplot/src/ticks.rs tests::ticks_are_ordered_inside_and_about_count`, `crates/glyphplot/src/downsample.rs tests::every_point_within_its_column`, `crates/glyphplot/src/downsample.rs tests::merged_equals_whole`, `crates/glyphplot/src/downsample.rs tests::lttb_keeps_ends_and_order`, `crates/glyphplot/src/cells.rs tests::column_and_x_agree`, `crates/glyphplot/src/layout.rs tests::labels_never_share_a_cell` | partial |

## 7. Charts

### 7.1 Line (LINE, SPARK)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-LINE-010 | v0.3.0 | `contract::every_chart_matches_its_snapshots` (`line`, `step-sine-1k`, `area-sine-1k`), `crates/glyphplot/src/charts/line/tests.rs tests::modes_draw_each_way` | partial |
| REQ-LINE-020 | v0.3.0 | `crates/glyphplot/src/charts/line/tests.rs tests::visible_slice_frames_are_fast` | yes |
| REQ-LINE-030 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 23) | yes |
| REQ-LINE-040 | v0.3.0 | `contract::hits_name_what_was_drawn` (rule 20), `crates/glyphplot/src/charts/line/mod.rs tests::readout_names_the_nearest_x` | yes |
| REQ-LINE-050 | v0.9.0 | `contract::every_chart_matches_its_snapshots` (`band`, `baseline`, stacked `area`), `crates/glyphplot/src/charts/line/mod.rs tests::percent_stack_sums_to_100` | — |
| REQ-SPARK-010 | v0.5.0 | `contract::every_chart_matches_its_snapshots` (`sparkline`), `crates/glyphplot/src/charts/line/spark/tests.rs tests::one_series_a_band_and_no_axes`, `crates/glyphplot/src/charts/line/spark/tests.rs tests::the_ring_keeps_the_last_capacity_rows`, `crates/glyphplot/src/charts/line/spark/tests.rs tests::text_parts_by_option_and_room`, `contract::streaming_equals_replace` (rule 14, v0.6.0) | partial |

### 7.2 Candles (CANDLE)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CANDLE-010 | v0.5.0 | `contract::every_chart_matches_its_snapshots` (`candle`, `hollow`, `ohlc`) | yes |
| REQ-CANDLE-020 | v0.5.0 | `contract::every_chart_matches_its_snapshots` (spacings 4, 2, 1), `crates/glyphplot/src/charts/candles/mod.rs tests::glyphs_by_spacing` | yes |
| REQ-CANDLE-030 | v0.5.0 | `contract::every_chart_keeps_the_contract` (rule 10), `crates/glyphplot/src/charts/candles/mod.rs tests::one_ink_resamples_below_four_cells` | yes |
| REQ-CANDLE-040 | v0.5.0 | `contract::every_chart_matches_its_snapshots` (`ohlcv-days`) | yes |
| REQ-CANDLE-050 | v0.5.0 | `contract::hits_name_what_was_drawn`, `crates/glyphplot/src/charts/candles/mod.rs tests::crosshair_snaps_to_ohlc` | yes |
| REQ-CANDLE-060 | v0.6.0 | `contract::streaming_equals_replace`, `crates/glyphplot/src/charts/candles/mod.rs tests::append_updates_the_last_candle` | — |
| REQ-CANDLE-070 | v0.9.0 | `contract::every_chart_matches_its_snapshots` (`heikin-ashi`) | — |

### 7.3 Bar (BAR)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-BAR-010 | v0.5.0 | `contract::every_chart_matches_its_snapshots`, `crates/glyphplot/src/charts/bar/mod.rs tests::free_end_at_an_eighth` | yes |
| REQ-BAR-020 | v0.5.0 | `crates/glyphplot/src/charts/bar/mod.rs tests::hit_by_band`, `contract::hits_name_what_was_drawn` | yes |
| REQ-BAR-030 | v0.9.0 | `crates/glyphplot/src/charts/bar/histogram.rs tests::counts_match_a_reference_binning` | — |

### 7.4 Scatter and heatmap (SCATTER, HEAT)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-SCATTER-010 | v0.9.0 | `contract::hits_name_what_was_drawn`, `crates/glyphplot/src/charts/scatter/index.rs tests::nearest_within_radius_matches_brute_force` | — |
| REQ-SCATTER-020 | v0.9.0 | `contract::every_chart_matches_its_snapshots` (`bubble`) | — |
| REQ-HEAT-010 | v0.9.0 | `contract::every_chart_matches_its_snapshots`, `contract::every_chart_keeps_the_contract` (rule 10) | — |
| REQ-HEAT-020 | v0.9.0 | `contract::every_chart_matches_its_snapshots` (`calendar`) | — |

## 8. The wasm boundary and the manifest

### 8.1 The boundary object (WASM)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-WASM-010 | v0.3.0 | `web/test/player.test.mjs` | yes |
| REQ-WASM-020 | v0.3.0 | `web/test/player.test.mjs` | yes |
| REQ-WASM-030 | v0.3.0 | `web/test/player.test.mjs` | yes |
| REQ-WASM-040 | v0.3.0 | `web/test/player.test.mjs` (debug build) | yes |
| REQ-WASM-050 | v0.3.0 | `web/test/memory.test.mjs` | yes |
| REQ-WASM-060 | v0.3.0 | `web/test/player.test.mjs` (release and debug builds) | yes |

### 8.2 Manifest and loading (MANIFEST, LOAD)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-MANIFEST-010 | v0.3.0 | `web/test/player.test.mjs`, `web/test/schemas.test.mjs`, `mise run size` | partial |
| REQ-MANIFEST-020 | v0.3.0 | `web/test/player.test.mjs` | yes |
| REQ-LOAD-010 | v0.3.0 | `web/test/player.test.mjs`, `site/e2e/element.spec.ts` | partial |
| REQ-LOAD-020 | v0.3.0 | `site/e2e/element.spec.ts` | partial |
| REQ-LOAD-030 | v0.3.0 | `site/e2e/element.spec.ts` | yes |

## 9. The element, the shell and rendering

### 9.1 The element (EL)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-EL-010 | v0.3.0 | `site/e2e/element.spec.ts`, `web/test/exports.test.mjs` | partial |
| REQ-EL-020 | v0.3.0 | `site/e2e/element.spec.ts` | yes |
| REQ-EL-030 | v0.5.0 | `site/e2e/element.spec.ts` | yes |
| REQ-EL-040 | v0.3.0 | `site/e2e/element.spec.ts` | partial |
| REQ-EL-050 | v0.5.0 | `site/e2e/element.spec.ts` | yes |
| REQ-EL-060 | v0.3.0 | `site/e2e/element.spec.ts` | partial |
| REQ-EL-070 | v0.3.0 | `site/e2e/element.spec.ts` | yes |
| REQ-EL-080 | v0.3.0 | `site/e2e/resize.spec.ts`, manual (devicePixelRatio on real screens) | yes |
| REQ-EL-085 | v0.3.0 | `site/e2e/resize.spec.ts` (a 6 px font drawn at the floor), `web/test/size.test.mjs` (the 9 px floor at 2×) | yes |
| REQ-EL-090 | v0.3.0 | `site/e2e/resize.spec.ts` | partial |
| REQ-EL-100 | v0.3.0 | `site/e2e/resize.spec.ts` | yes |
| REQ-EL-110 | v0.5.0 | `site/e2e/resize.spec.ts` | yes |
| REQ-EL-120 | v0.3.0 | `site/e2e/element.spec.ts` | partial |
| REQ-EL-130 | v0.3.0 | `site/e2e/element.spec.ts` | yes |
| REQ-EL-140 | v0.3.0 | `site/e2e/element.spec.ts` | yes |
| REQ-EL-150 | v0.3.0 | `site/e2e/element.spec.ts` | yes |

### 9.2 Mount and the JS API (MOUNT, JS, SSR)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-MOUNT-010 | v0.3.0 | `site/e2e/element.spec.ts` | partial |
| REQ-MOUNT-020 | v0.3.0 | `site/e2e/element.spec.ts` | partial |
| REQ-JS-010 | v0.3.0 | `web/test/player.test.mjs`, `web/test/snapshot.test.mjs` | partial |
| REQ-SSR-010 | v0.8.0 | `web/test/exports.test.mjs` | — |

### 9.3 Rendering (RENDER)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-RENDER-010 | v0.3.0 | `mise run bench:render` | partial |
| REQ-RENDER-020 | v0.3.0 | `web/test/blocks.test.mjs`, manual (pixels) | partial |
| REQ-RENDER-030 | v0.3.0 | `web/test/render.test.mjs` | partial |
| REQ-RENDER-040 | v0.5.0 | `web/test/render.test.mjs`, manual (pixels) | yes |
| REQ-RENDER-050 | v0.3.0 | manual (Chrome and Safari measurements), review (the plan records them) | — |

### 9.4 Font (FONT)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-FONT-010 | v0.2.0 | `web/test/font.test.mjs`, `mise run build:font` | yes |
| REQ-FONT-020 | v0.5.0 | `web/test/font.test.mjs` | yes |
| REQ-FONT-030 | v0.3.0 | `site/e2e/element.spec.ts`, manual (fonts) | partial |

## 10. Theme and one ink

### 10.1 Theme (THEME)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-THEME-010 | v0.3.0 | `web/test/theme.test.mjs`, `site/e2e/element.spec.ts` (no new chart on a theme change), `site/e2e/theme.spec.ts` (planned) | partial |
| REQ-THEME-020 | v0.3.0 | `web/test/theme.test.mjs`, `site/e2e/element.spec.ts` (no new chart on a theme change), `site/e2e/theme.spec.ts` (planned) | partial |
| REQ-THEME-030 | v0.3.0 | `web/test/theme.test.mjs`, `site/e2e/element.spec.ts` (no new chart on a theme change), `site/e2e/theme.spec.ts` (planned) | partial |
| REQ-THEME-040 | v0.3.0 | `crates/glyphplot/src/theme.rs tests::default_tokens_meet_contrast_bars` | yes |
| REQ-THEME-050 | v0.3.0 | `crates/glyphplot/src/theme.rs tests::default_colours_stay_apart_under_cvd` | yes |
| REQ-THEME-060 | v0.5.0 | `contract::every_chart_keeps_the_contract` (rule 10), `contract::every_chart_matches_its_snapshots` | partial |
| REQ-THEME-065 | v0.5.0 | `crates/glyphplot/src/theme.rs tests::default_colours_stay_apart_under_cvd`, `web/test/theme.wasm.test.mjs` | yes |
| REQ-THEME-070 | v0.3.0 | `crates/glyphplot/src/theme.rs tests::low_contrast_series_warn`, `web/test/theme.test.mjs` | partial |
| REQ-THEME-080 | v0.9.0 | `crates/glyphplot/src/theme.rs tests::default_scales_are_monotonic_and_separate` | — |
| REQ-THEME-090 | v0.8.0 | `crates/cli/tests/cli.rs::theme_file_is_read`, `web/test/exports.test.mjs`, `crates/glyphplot/src/export.rs tests::exports_take_a_theme` | — |
| REQ-THEME-100 | v0.7.0 | `site/e2e/theme.spec.ts` (forced colours, print), manual (Windows high contrast) | — |

### 10.2 One ink (MONO)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-MONO-010 | v0.3.0 | `contract::every_chart_keeps_the_contract` (rule 11), `crates/glyphplot/src/mono.rs tests::dash_patterns_follow_the_origin` | yes |
| REQ-MONO-020 | v0.5.0 | `contract::every_chart_keeps_the_contract` (rule 11) | partial |
| REQ-MONO-030 | v0.5.0 | `contract::every_chart_keeps_the_contract` (rule 11), `contract::every_chart_matches_its_snapshots` | yes |
| REQ-MONO-040 | v0.3.0 | `contract::every_chart_matches_its_snapshots`, `contract::every_chart_keeps_the_contract` (rule 11) | yes |
| REQ-MONO-050 | v0.9.0 | `contract::every_chart_matches_its_snapshots` | — |

## 11. Accessibility

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-A11Y-010 | v0.3.0 | `site/e2e/a11y.spec.ts` | partial |
| REQ-A11Y-020 | v0.3.0 | `site/e2e/a11y.spec.ts`, `crates/glyphplot/src/table.rs tests::table_keeps_extremes_under_500_rows` | partial |
| REQ-A11Y-030 | v0.3.0 | `site/e2e/a11y.spec.ts`, `site/e2e/interaction.spec.ts`, `web/test/a11y.test.mjs`, `crates/glyphplot/src/charts/line/tests.rs tests::keys_do_what_the_table_says` | yes |
| REQ-A11Y-040 | v0.3.0 | `site/e2e/interaction.spec.ts` | partial |
| REQ-A11Y-050 | v0.3.0 | `site/e2e/a11y.spec.ts` (under reduced motion a view change jumps) | yes |
| REQ-A11Y-060 | v0.3.0 | `site/e2e/a11y.spec.ts`, manual (focus outline) | yes |
| REQ-A11Y-070 | v0.3.0 | `site/e2e/a11y.spec.ts` | yes |
| REQ-A11Y-080 | v0.6.0 | `site/e2e/streaming.spec.ts`, `web/test/stream.wasm.test.mjs` | yes |
| REQ-A11Y-090 | v0.6.0 | `site/e2e/streaming.spec.ts` | yes |
| REQ-A11Y-100 | v0.6.0 | `site/e2e/streaming.spec.ts`, `web/test/a11y.test.mjs`, manual (screen readers: test.html case 32) | partial |
| REQ-A11Y-110 | v0.7.0 | `site/e2e/a11y.spec.ts` | — |
| REQ-A11Y-120 | v0.8.0 | `crates/cli/tests/cli.rs::table_prints_the_data` | — |

## 12. Budgets

### 12.1 Size (SIZE)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-SIZE-010 | v0.1.0 | `mise run size` | yes |
| REQ-SIZE-020 | v0.1.0 | `mise run size` | yes |
| REQ-SIZE-030 | v0.1.0 | `mise run size` (`WASM_GZ_LIMIT`) | yes |
| REQ-SIZE-040 | v0.1.0 | `mise run size` | yes |
| REQ-SIZE-050 | v0.4.0 | `mise run size --charts` | yes |
| REQ-SIZE-060 | v0.2.0 | `mise run lint` (`cargo tree -e normal --target wasm32-unknown-unknown` ban list) | yes |
| REQ-SIZE-070 | v0.10.0 | `mise run size` (pack modules) | — |

### 12.2 Time (PERF)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-PERF-010 | v0.3.0 | `contract::downsampling_is_exact` (rule 27) | yes |
| REQ-PERF-020 | v0.3.0 | `crates/glyphplot/src/charts/line/tests.rs tests::unchanged_frames_read_no_data_and_draw_no_base` | partial |
| REQ-PERF-030 | v0.4.0 | `budgets::every_chart_keeps_its_time_budgets` | yes |
| REQ-PERF-040 | v0.4.0 | `budgets::every_chart_keeps_its_time_budgets` | yes |
| REQ-PERF-050 | v0.4.0 | `budgets::every_chart_keeps_its_time_budgets`, `mise run bench:web` | yes |
| REQ-PERF-060 | v0.4.0 | `mise run bench:web` | yes |
| REQ-PERF-070 | v0.6.0 | `mise run bench:web` (the streaming row) | yes |
| REQ-PERF-080 | v0.4.0 | `mise run bench:load` | yes |

### 12.3 Memory (MEM)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-MEM-010 | v0.3.0 | `web/test/memory.test.mjs` | yes |
| REQ-MEM-020 | v0.3.0 | `web/test/data.test.mjs`, `web/test/data.wasm.test.mjs` | yes |
| REQ-MEM-030 | v0.3.0 | `web/test/memory.test.mjs` | yes |

### 12.4 Benches (BENCH)

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-BENCH-010 | v0.4.0 | `mise run bench` | yes |
| REQ-BENCH-020 | v0.4.0 | `mise run bench:web`, `mise run bench:load`, `mise run bench:render` | yes |

## 13. Testing

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-TEST-010 | v0.1.0 | `mise run check`, review (process) | partial |
| REQ-TEST-020 | v0.3.0 | `mise run test:web` (alias `test:wasm`), `web/test/player.test.mjs` (debug build) | yes |
| REQ-TEST-030 | v0.3.0 | manual (itself) | — |
| REQ-TEST-040 | v0.3.0 | manual (the owner's pass on `site/test.html`), review (the page) | — |
| REQ-E2E-010 | v0.3.0 | `mise run test:site` | yes |
| REQ-E2E-020 | v0.3.0 | `site/e2e/interaction.spec.ts` (itself) | yes |
| REQ-E2E-030 | v0.3.0 | `site/e2e/element.spec.ts` (itself) | yes |
| REQ-E2E-040 | v0.3.0 | `site/e2e/resize.spec.ts` (itself) | yes |
| REQ-E2E-050 | v0.6.0 | `site/e2e/streaming.spec.ts` (itself) | yes |
| REQ-E2E-060 | v0.7.0 | `site/e2e/charts.spec.ts` | — |
| REQ-E2E-070 | v0.3.0 | `mise run check`, `mise run signoff` | partial |

## 14. The CLI

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CLI-010 | v0.8.0 | `crates/cli/tests/cli.rs::usage_errors_are_rejected` | — |
| REQ-CLI-020 | v0.8.0 | `crates/cli/tests/cli.rs::exit_codes_and_messages` | — |
| REQ-CLI-030 | v0.8.0 | `crates/cli/tests/cli.rs::unknown_type_suggests_up_to_eight` | — |
| REQ-CLI-040 | v0.8.0 | `crates/cli/tests/terminal.rs::stdin_streams_into_the_chart` | — |
| REQ-CLI-050 | v0.8.0 | `crates/cli/tests/terminal.rs::stdin_streams_into_the_chart`, `crates/cli/tests/cli.rs::window_keeps_the_last_n_rows` | — |
| REQ-CLI-060 | v0.8.0 | `crates/cli/tests/terminal.rs::piped_it_prints_one_plain_frame`, `crates/cli/tests/cli.rs::closed_stdout_exits_0` | — |
| REQ-CLI-070 | v0.8.0 | `crates/cli/tests/terminal.rs::mouse_pans_and_zooms` | — |
| REQ-CLI-080 | v0.8.0 | `crates/cli/src/color.rs tests::color_auto_follows_the_environment` | — |
| REQ-CLI-090 | v0.8.0 | `crates/cli/tests/terminal.rs::a_key_stops_it_and_gives_the_terminal_back`, `crates/cli/tests/terminal.rs::ctrl_c_stops_it_with_130_and_gives_the_terminal_back`, `crates/cli/tests/terminal.rs::sigterm_stops_it_and_gives_the_terminal_back`, `crates/cli/tests/terminal.rs::sighup_stops_it_and_gives_the_terminal_back`, `crates/cli/tests/terminal.rs::a_panic_gives_the_terminal_back` | — |
| REQ-CLI-100 | v0.8.0 | `crates/cli/tests/cli.rs::demo_says_the_data_is_generated` | — |
| REQ-CLI-110 | v0.8.0 | `crates/cli/tests/cli.rs::glyphs_convert_every_frame` | — |
| REQ-CLI-120 | v0.8.0 | `crates/cli/tests/terminal.rs::frames_write_only_changed_runs` | — |
| REQ-TERM-010 | v0.8.0 | `crates/cli/tests/terminal.rs` (itself: the seven cases of `discovery/testing.md`) | — |

## 15. The Rust crate and exports

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-CRATE-010 | v0.8.0 | `crates/glyphplot/src/export.rs tests::renders_text_and_svg`, `mise run lint` (`cargo tree -e normal -p glyphplot`) | — |
| REQ-CRATE-020 | v0.11.0 | `crates/ratatui/tests/widget.rs::writes_every_cell_in_its_area` | — |
| REQ-EXPORT-010 | v0.8.0 | `web/test/exports.test.mjs`, `crates/cli/tests/cli.rs::render_matches_the_element` | — |
| REQ-EXPORT-020 | v0.8.0 | `crates/glyphplot/src/export.rs tests::export_draws_the_current_view`, `web/test/exports.test.mjs` | — |
| REQ-EXPORT-030 | v0.8.0 | `crates/cli/tests/cli.rs::png_renders`, `mise run lint` (`cargo tree`: no `png` feature in wasm) | — |

## 16. The site and datasets

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-SITE-010 | v0.7.0 | `mise run build:site`, `site/e2e/charts.spec.ts` | — |
| REQ-SITE-020 | v0.7.0 | `site/e2e/charts.spec.ts` | — |
| REQ-SITE-030 | v0.7.0 | `site/e2e/site.spec.ts` | — |
| REQ-SITE-040 | v0.7.0 | `site/e2e/site.spec.ts` | — |
| REQ-SITE-050 | v0.7.0 | `site/e2e/site.spec.ts` (feed down), review (source and licence named) | — |
| REQ-SITE-060 | v0.7.0 | `site/e2e/site.spec.ts` | — |
| REQ-SITE-070 | v0.7.0 | `site/e2e/a11y.spec.ts` | — |
| REQ-SITE-080 | v0.7.0 | `mise run examples:check`, `site/e2e/charts.spec.ts` (every example's page) | — |
| REQ-SITE-090 | v0.7.0 | `mise run examples:check` (the example count) | — |
| REQ-SITE-100 | v0.8.0 | `mise run examples:check` (on each milestone's branch) | — |
| REQ-SITE-110 | v0.7.0 | `mise run examples:check` (generated data labelled with its seed), `site/e2e/site.spec.ts` | — |
| REQ-SITE-120 | v0.7.0 | `site/e2e/site.spec.ts` (no wasm fetched before a scroll; one fetch a family) | — |
| REQ-DATASET-010 | v0.7.0 | `mise run datasets:check` | — |
| REQ-DATASET-020 | v0.7.0 | `mise run datasets:check` | — |
| REQ-DATASET-030 | v0.7.0 | `mise run datasets:check`, `web/test/package.test.mjs` | — |

## 17. Domain packs

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-PACK-010 | v0.10.0 | `mise run lint` (`cargo tree` per pack: no chart) | — |
| REQ-PACK-020 | v0.10.0 | `mise run size --charts` (symbol check) | — |
| REQ-PACK-030 | v0.10.0 | `crates/packs/<domain>/tests/presets.rs::every_preset_matches_its_snapshot` | — |
| REQ-PACK-040 | v0.10.0 | `web/test/packs.test.mjs` | — |
| REQ-PACK-050 | v0.10.0 | `crates/packs/<domain>/tests/streaming.rs::time_a_sample_is_flat` | — |
| REQ-MKT-010 | v0.10.0 | `crates/packs/markets/src/indicators.rs tests::batch_and_incremental_agree` | — |
| REQ-MKT-020 | v0.10.0 | `crates/packs/markets/src/indicators.rs tests::nan_until_the_window_is_full` | — |
| REQ-MKT-030 | v0.10.0 | `crates/packs/markets/src/aggregate.rs tests::trades_aggregate_to_ohlcv` | — |
| REQ-MKT-040 | v0.10.0 | `crates/packs/markets/src/synth.rs tests::same_seed_same_output`, `crates/packs/markets/tests/presets.rs::synthetic_presets_say_simulated` | — |
| REQ-TELE-010 | v0.10.0 | `crates/packs/telemetry/src/prometheus.rs tests::reads_query_range_with_special_values` | — |
| REQ-TELE-020 | v0.10.0 | `crates/packs/telemetry/src/rate.rs tests::a_drop_is_a_reset` | — |
| REQ-TELE-030 | v0.10.0 | `crates/packs/telemetry/src/quantile.rs tests::matches_histogram_quantile_examples` | — |
| REQ-TELE-040 | v0.10.0 | `crates/packs/telemetry/src/prometheus.rs tests::stale_becomes_nan` | — |

## 18. Long tail: sky and orbits

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-ASTRO-010 | v0.12.0 | `crates/packs/astronomy/tests/horizons.rs::planets_within_3_arcmin` | — |
| REQ-ASTRO-020 | v0.12.0 | `crates/packs/astronomy/src/coords.rs tests::altaz_within_a_tenth_of_a_degree` | — |
| REQ-ASTRO-030 | v0.12.0 | `crates/packs/astronomy/src/precess.rs tests::precesses_j2000`, `crates/packs/astronomy/src/fold.rs tests::folds_into_half_phase`, `crates/packs/astronomy/src/horizons.rs tests::parses_the_soe_block` | — |
| REQ-SKY-010 | v0.12.0 | `contract::every_chart_matches_its_snapshots` (`sky`), `crates/glyphplot/src/charts/sky/mod.rs tests::icrs_to_altaz_within_a_tenth_of_a_degree` | — |
| REQ-SKY-020 | v0.12.0 | `mise run size --charts` (symbol check) | — |
| REQ-SKY-030 | v0.12.0 | `contract::every_chart_matches_its_snapshots` (`orbit`), `crates/glyphplot/src/charts/sky/orbit.rs tests::kepler_positions` | — |

## 19. Packaging

| Requirement | Milestone | Planned check | Status |
|---|---|---|---|
| REQ-PKG-010 | v0.11.0 | `web/test/package.test.mjs` | — |
| REQ-PKG-020 | v0.11.0 | `web/test/package.test.mjs` | — |
| REQ-PKG-030 | v0.11.0 | `mise run lint` (`tsc` on the JSX fixture) | — |
| REQ-PKG-040 | v0.1.0 | review (crate names in each `Cargo.toml`) | — |
| REQ-PKG-050 | v0.11.0 | `web/test/package.test.mjs`, review (`package.json`) | — |
| REQ-PKG-060 | v0.7.0 | `site/e2e/site.spec.ts`, review (install snippets) | — |

## Summary

340 requirements.

### By milestone

| Milestone | Requirements |
|---|---|
| v0.1.0 | 20 |
| v0.2.0 | 47 |
| v0.3.0 | 122 |
| v0.4.0 | 13 |
| v0.5.0 | 37 |
| v0.6.0 | 16 |
| v0.7.0 | 18 |
| v0.8.0 | 25 |
| v0.9.0 | 14 |
| v0.10.0 | 14 |
| v0.11.0 | 5 |
| v0.12.0 | 9 |
| **Total** | **340** |

### By kind of planned check

"Any check" counts a requirement under every kind its row lists, so that
column sums to more than the total. "First check" counts it once, under the
kind of the first check listed.

| Kind | Any check | First check |
|---|---|---|
| Rust | 183 | 176 |
| Node | 67 | 43 |
| browser | 76 | 60 |
| task/lint | 64 | 52 |
| manual/review | 33 | 9 |
| **Total** | — | **340** |

### By status

| Status | Requirements |
|---|---|
| yes | 176 |
| partial | 62 |
| — | 102 |
