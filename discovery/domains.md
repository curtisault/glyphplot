# Domain packs

Discovery agent 7 of 10. Topic: `README.md`, "The briefs", topic 7. Written 2026-10-18.

## Summary

- Every domain the owner named reduces to a small set of chart types, scales and formatters that belong in the library, plus domain arithmetic that does not: counter rates, histogram quantiles, OHLCV aggregation, indicators, trading calendars, flow aggregation, planet positions, precession, alt/az, phase folding. That arithmetic goes in a domain pack.
- A domain pack is a Rust crate that depends on the core's data contract and on no chart. On the web it is its own wasm module; in the CLI and the Rust crate it links natively. It writes columns that a chart then draws. It never draws.
- Packs are small. A probe with eight planets, RA/Dec and alt/az built to 6,460 bytes gzipped; a probe with SMA, EMA, Bollinger, RSI and MACD built to 1,454 bytes gzipped (see "Measurements"). Size does not decide where a pack's code lives. Separation, one implementation for web and terminal, and streaming cost do.
- Indicators belong in the markets pack, in Rust, computed incrementally. Recomputing all five over 1,000,000 samples costs 1.2 to 26.5 ms each in Node; appending one sample is O(1) per indicator. Handing a column from the pack's memory to a chart's memory costs 0.127 ms for 1,000,000 values.
- The library should not compute celestial positions. The astronomy pack should, from JPL's published approximate Keplerian elements. Against Horizons over 2026 these are within 35″ for Mars and 123″ for Jupiter as seen from Earth. That is at most 6% of one braille dot on a whole-sky chart 320 columns wide. Horizons stays the source for precise tables and for the Moon, comets and asteroids. VSOP87 buys nothing a cell can show.
- Market data is the licensing trap the brief expected. Coinbase's terms (2026-08-07) forbid showing its data or charts derived from it to anyone outside your organisation. Binance's dataset terms (2026-08-26) are CC BY-NC-SA 4.0 with further prohibitions. Kraken requires permission for non-personal commercial use. No exchange feed is safe for a public site without written permission. Safe price data is synthetic, or a public-domain US government series fetched from its publisher: EIA's WTI spot price, Treasury yields, the Federal Reserve's exchange rates.
- First packs: telemetry (v0.7.0) and markets (v0.7.0), then astronomy (v0.8.0). Network and geoscience start as example pages over existing charts. The site's live candlestick runs on a labelled synthetic feed, and a second live candlestick shows a real public feed: Wikimedia edits per second, one candle a minute.

## Scope and method

- Read: the brief (`README.md`, "The briefs", topic 7); unicel's `spec/ROADMAP.md` §6.2 (the data shapes, REQ-DATA-004); `crates/unicel/src/pieces/data/{candlesticks,bar_chart,sparkline}.rs`; the headers of every piece in `crates/unicel/src/pieces/space/`.
- Read the terms of each data source on its publisher's own page, quoted below with the date the page states. Checked CORS headers with `curl -H "Origin: https://example.org"`, because a live example on the site can only read a feed that sends `access-control-allow-origin`.
- Downloaded a sample of each candidate dataset into the session scratchpad, not the repo. Recorded its size, row count and SHA-256 (`shasum -a 256`). The repo stores no data.
- Built throwaway probes under `discovery/bench/domains/` (README there): a Keplerian-elements check against Horizons, and a `no_std` wasm module for planet positions and indicators, measured for size and speed. Machine: Apple M5 Pro, Rust 1.97, Node 26.5.1.
- Not covered: pcap parsing cost and size; graph layout for topology; VSOP87 and lunar theory sizes; Safari and Chrome timings (Node only); sports data sources beyond noting that they are rarely licensed for reuse; the Internet Topology Zoo's licence (its site refused connections on 2026-10-18).

## Measurements

| What | Result | How |
|---|---|---|
| Keplerian elements vs Horizons, Mars, 2026, daily (366 days) | heliocentric max 68″, mean 41″; geocentric max 35″, mean 20″ | `discovery/bench/domains/kepler_vs_horizons.py` with JPL Table 1 (1800–2050) elements against Horizons `VECTORS`, centre `500@10`, ecliptic J2000. Geocentric uses the Earth–Moon barycentre elements for Earth. |
| Same, Jupiter | heliocentric max 97″, mean 51″; geocentric max 123″ (2.0′), mean 46″ | same |
| One braille dot on a whole-sky chart | 2,025″ (360° over 320 columns × 2 dots) | arithmetic |
| Same probe in Rust/wasm, Mars 2026-01-01 | RA −76.510° (283.490°), Dec −23.752°; Horizons geometric 283.494°, −23.751° | `discovery/bench/domains/check.mjs` |
| Astronomy probe (8 planets, RA/Dec, alt/az, `libm`) | 11,605 B raw, 6,460 B gzip −9 | `discovery/bench/domains/sizes`, `--features astro`, unicel's release profile, no wasm-bindgen, no wasm-opt |
| Indicator probe (SMA, EMA, Bollinger, RSI, MACD) | 2,211 B raw, 1,454 B gzip −9 | same, `--features indicators` |
| Both | 12,885 B raw, 7,051 B gzip −9 | same, `--features astro,indicators` |
| Indicators over 1,000,000 f64, wasm in Node, mean of 10 after a warm-up | SMA 1.2 ms; EMA 2.4 ms; Bollinger 6.7 ms in one memory layout, 26.0–26.5 ms in another (cause not found); RSI 3.6 ms; MACD 1.9 ms | `check.mjs`, two runs |
| Copy a 1,000,000-value `Float64Array` from one wasm memory to another | 0.127 ms (mean of 50); one value 0.032 µs | Node, `TypedArray.set` |
| HYG v4.1 naked-eye subset (mag ≤ 6.5; ra, dec, mag, ci, proper, con) | 8,920 stars; 305,537 B CSV; 118,362 B gzip −9 | script in the scratchpad over `hygdata_v41.csv` |

The Keplerian error is about one-sixteenth of a dot on a whole-sky chart, and still under one dot when the chart is zoomed ten times (a 36° field at 202″ a dot).

## Telemetry

What a user wants to see:

- A metric over time: CPU, memory, temperature, queue depth. This is a line or area chart, many series, with a legend that toggles them.
- A rate from a counter: requests per second from `http_requests_total`, with counter resets handled. This is a line chart of derived values.
- Latency: p50, p90 and p99 over time as lines or a band, on a log scale because latency spans decades. Also a latency heatmap: time on x, log-spaced buckets on y, count as colour. This is the shape Prometheus and OpenTelemetry histograms carry.
- SLOs: a target line, a shaded band for the error budget, burn rate, and annotations for incidents and deploys.
- Status: uptime bars per service (unicel's `uptime` shape, ROADMAP §6.2), sparklines in a table, a single gauge.

The data shapes:

- Prometheus `/api/v1/query_range` returns a matrix. Each series has `metric` labels and `values` pairs `[unix_seconds, "string value"]`. Values are strings because JSON cannot carry NaN or ±Inf. Native histograms come as `histograms` pairs, whose buckets are `[boundary_rule, left, right, count]` (https://prometheus.io/docs/prometheus/latest/querying/api/).
- Classic Prometheus histograms are `_bucket` series with `le` labels, plus `_sum` and `_count`. Native histograms use schema −4 to +8, base 2^(2^−n), and schema −53 for custom bounds. A counter has reset when its value drops; a histogram has reset when its count or any bucket drops (https://prometheus.io/docs/specs/native_histograms/).
- OpenTelemetry metrics are Sum (monotonic or not, delta or cumulative), Gauge, Histogram (explicit bounds, optional min and max), ExponentialHistogram (base = 2^(2^−scale), with `zero_count`), and legacy Summary. A "no recorded value" flag marks explicit gaps, the counterpart of Prometheus staleness (https://opentelemetry.io/docs/specs/otel/metrics/data-model/).

The split:

| Need | Where | Why |
|---|---|---|
| Line, area, band (lower and upper column), step, heatmap with non-uniform y bins | library, chart types | Every domain uses them. |
| Log y scale; time x scale with zone | library, scale | Latency spans decades. |
| Gaps: NaN breaks a line; a "no value" flag breaks it too | library, data contract | Staleness must show as a gap, not a line drawn through it. |
| Formatters: SI (k, M, G), IEC bytes (KiB, MiB), bits per second, durations (ns, µs, ms, s), percent | library, formatter | Every axis and readout needs them; they are a few hundred lines. |
| Threshold lines and shaded bands as annotations | library, annotation | SLOs, Bollinger, and the habitable zone on an orbit plot are the same thing. |
| Prometheus matrix JSON and OTLP/JSON metrics into columns; counter to rate with reset handling; bucket counts to quantiles by linear interpolation within the bucket (as PromQL's `histogram_quantile` does, **(unverified)**: the native histogram spec page was read only to 100,000 characters); delta to cumulative | telemetry pack | Domain arithmetic, and the CLI needs the same code: `glyphplot line --from prom.json`. |
| Presets: "latency" (log y, ms formatter, p50/p90/p99 colours), "rate", "saturation" (0–100 %) | telemetry pack | Options objects, no code. |
| Fetching from a Prometheus server, auth, polling | example page | Transport is the page's. |

## Network

What a user wants to see:

- Throughput in and out over time, in bits per second, per interface. This is an area chart, mirrored (in above the axis, out below) or stacked.
- Packets per second, retransmits, round-trip time (a band of min, median and max), loss.
- Top talkers and top ports: a bar chart of the top N flows, re-ranked as the window moves.
- Flows between hosts: Sankey or chord charts (long tail in agent 1's taxonomy).
- Topology: nodes and edges with weights. This is a graph chart, long tail.
- A packet capture's timeline: a TCP time-sequence plot (the scatter Wireshark draws, sequence number against time) and an inter-arrival histogram.

The split:

| Need | Where |
|---|---|
| Mirrored area (series below the axis); stacked area; bar with re-ranking; scatter at 1M points | library |
| Bits per second and bytes formatters (SI for rates, IEC for sizes) | library |
| Graph data shape (nodes, edges, weights), in the data contract | library (agent 2's contract), drawn by a later graph chart |
| Force or layered layout for topology | its own chart module when it comes; layout is not a pack's job because the chart needs positions for hit testing |
| pcap and pcapng parsing; flow aggregation (5-tuple, bytes, packets, first and last seen); top-N; TCP sequence extraction | network pack, later than the first packs, because a pcap reader is the largest pack and its size is unmeasured |
| SNMP or Prometheus interface counters to rates | telemetry pack, not network: they are counters |

Datasets: no third-party network dataset checked is safe to redistribute. The Wireshark wiki's sample captures state no licence (https://wiki.wireshark.org/SampleCaptures, first 100,000 characters read). RIPE NCC's RIS and Atlas data come under terms that restrict redistribution and grant only a revocable permission for commercial use (https://www.ripe.net/about-us/legal/ripe-data-repository-terms-and-conditions/, https://ripe.net/analyse/internet-measurements/routing-information-service-ris/commercial-use). The network examples use synthetic traffic generated from a seed in demo mode, and the user's own captures through the CLI. NAB's `realAWSCloudwatch` series include network bytes in, under MIT (Datasets).

## Markets

What a user wants to see:

- OHLCV candles with volume in a second pane below, sharing the x axis. This is the owner's one-line test.
- Overlays on the price pane: SMA, EMA, Bollinger bands (a band and a mid line).
- Oscillators in panes of their own: RSI (0–100, lines at 30 and 70), MACD (line, signal, histogram about zero).
- Order-book depth: cumulative bid and ask volume against price. This is a step area in two colours meeting at the mid price.
- Session gaps: no empty space for nights, weekends or holidays. TradingView's Lightweight Charts uses an index-based ("logical") time scale: "The logical range starts from the first data point across all series" and indices can be fractional (https://tradingview.github.io/lightweight-charts/docs/time-scale). The page implies that a gap in time reserves no space; it does not say so.
- A live last price line, a crosshair readout of O, H, L, C, V and change, and scrubbing.
- Beyond markets: yield curves (maturity × date as a heatmap, or one curve scrubbed through time), exchange rates, a loan's amortisation, a budget's burn-down. These are line, area and bar charts with currency and percent formatters.

The split:

| Need | Where | Why |
|---|---|---|
| Candlestick chart with OHLC and HLC-bar modes; volume bars; panes sharing an x axis | library | The owner's first chart. |
| An index time scale: x is the bar index, ticks are labelled from each bar's timestamp, and gaps in time take no space | library, scale | Every price chart needs it, and the pointer-to-datum mapping (agent 3) depends on it. It needs only the timestamps in the data, not a calendar. |
| Price formatter with a tick size (0.01, 0.25, 1/32), currency, percent change with sign | library, formatter | Readouts and axes. |
| Step area (for depth) | library, an area mode | Also used for histograms and stairs. |
| Indicators: SMA, EMA, Bollinger, RSI, MACD, VWAP, ATR | markets pack | See below. |
| Aggregating trades or ticks into OHLCV candles of a period; resampling 1 m to 5 m, 1 h, 1 d | markets pack | Domain arithmetic, needed for every live feed. Exchanges send trades, not candles. |
| Trading calendars (exchange hours, holidays, half days) | markets pack, data only, later | Needed only to show a gap where a bar is missing. The index scale already removes non-trading time. |
| Order-book snapshots and diffs into cumulative depth columns | markets pack | Domain state. |
| Synthetic market generator for demo mode | markets pack | unicel's `candlesticks.rs` already has a seeded walk (Mulberry32, after ascii.rest, MIT). |
| Exchange WebSocket wiring | example page, shown as code and not run on the public site | Licence, below. |

### Where indicators are computed

| Option | Size | Determinism | Streaming cost | Terminal |
|---|---|---|---|---|
| In the chart module (the candlestick chart takes `{"overlay":[{"sma":20}]}`) | +1,454 B gzip per chart module that has them (measured) | Rust f64, deterministic | O(1) a sample, no copy | same code |
| In a markets pack module that writes columns | 1,454 B gzip once, in a module only market pages load | Rust f64, deterministic | O(1) a sample, plus one copy of the new values into the chart's memory (0.032 µs a value, 0.127 ms for 1,000,000) | same code, linked natively |
| In TypeScript in the shell or an example | none in wasm; a few KB of JS **(unverified)** | JS doubles are IEEE 754 too, but a second implementation would have to match the Rust CLI's output bit for bit and be tested for it | O(1) a sample, plus a copy | the CLI needs a second implementation in Rust |

Recommendation: the markets pack, in Rust.

- The CLI and the Rust crate need indicators too, so a TypeScript version would be a second implementation.
- The RSI and MACD panes are line and bar charts, not candlestick charts, so putting indicators in the candlestick module would not even serve the panes that draw them.
- The brief says a pack must not bloat a chart module.
- The pack keeps O(window) state per indicator and exposes `push(sample) -> values` and `compute(column) -> column`.
- The size is measured to be trivial, so the boundary is drawn for structure, not bytes.

### Market data licences

| Source | Terms | Safe for a public repo? | Safe for a public site? |
|---|---|---|---|
| Coinbase Exchange market data, REST and WebSocket | Market Data Terms of Use, last updated 2026-08-07. The licence is "exclusively for you or your entity's personal or research purposes and may not be used to build an application intended for use by end users", and you may not "Redistribute, display, or disseminate the Market Data—or any data, charts, analytics … derived from the Market Data … to any third party" without written consent (https://www.coinbase.com/legal/market_data). | no | no, not even a reader's browser connecting to it from the site |
| Binance Vision historical datasets (data.binance.vision) | Vision Dataset Terms v1.0, 2026-08-26: CC BY-NC-SA 4.0, plus "You shall not … host … the Datasets or any derivative real-time or historical data feeds". The terms extend to "charts, models, and derivative analytical indicators" (https://data.binance.vision/Binance_Vision-Terms_of_Use.pdf). | no (NC and SA conflict with MIT) | no |
| Kraken public WebSocket (`wss://ws.kraken.com/v2`) | The API guide requires prior permission for "non-personal commercial use of public market data", contact marketdata@kraken.com (https://docs.kraken.com/api/docs/guides/global-intro, via search snippet; the full terms page was not read). | no | only with written permission |
| FRED (St. Louis Fed) | Each series carries its own status. "Public Domain: Citation Requested" series "may be used without permission". Copyrighted series need the owner's permission; S&P 500 (`SP500`) says "Reproduction of S&P 500 in any form is prohibited". FRED's own terms also forbid providing "any stored, cached, or archived portion of the FRED® Services or FRED® Content to any third party", and its API forbids caching (https://fred.stlouisfed.org/legal/). | take public-domain series from the original publisher, not from FRED | same |
| EIA (WTI spot `RWTC`, daily since 1986) | "U.S. government publications are in the public domain and are not subject to copyright protection"; cite "Source: U.S. Energy Information Administration (date)" (https://www.eia.gov/about/copyrights_reuse.php). | yes | yes |
| U.S. Treasury daily par yield curve | US government work, public domain (17 U.S.C. §105; the Treasury page states no licence). The CSV sends `access-control-allow-origin: *`. | yes | yes, live |
| Federal Reserve H.10 exchange rates (e.g. USD/EUR; on FRED as `DEXUSEU`, "Public Domain: Citation Requested") | public domain | yes, fetched from federalreserve.gov | yes |
| Synthetic OHLCV from a seed | the project's own, MIT | yes | yes |

There is no genuinely redistributable intraday OHLCV for a real traded instrument among the sources checked. The public-domain series are daily closes, and EIA and Treasury give no volume. Weekly candles built from a daily close series (open is the first close of the week, high and low the extremes, close the last) are honest, and demonstrate aggregation. They have no volume. The site's volume pane therefore runs on synthetic data, labelled as synthetic.

Live feeds the site can connect to under their terms:

- Wikimedia EventStreams (`https://stream.wikimedia.org/v2/stream/recentchange`, server-sent events, `access-control-allow-origin: *`). It is "intended for use by small scale external tool developers"; the server closes connections after 15 minutes and clients reconnect (https://wikitech.wikimedia.org/wiki/Event_Platform/EventStreams_HTTP_Service). The page states no licence for the event data. Counting events per second uses facts, not content. A live candlestick of edits per second, one candle a minute, passes the owner's one-line test with a real feed and no market licence.
- USGS real-time earthquake GeoJSON (CORS `*`, `cache-control: max-age=60`). See Geoscience.
- NOAA SWPC planetary K-index JSON (`https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json`, 4,330 B, CORS `*`) and NOAA CO-OPS water levels (CORS `*`). These are US government works.

## Astronomy

What a user wants to see:

- A sky chart: stars by RA and Dec, sized by magnitude, coloured by B−V colour index, with constellation lines and labels. Whole sky in an equirectangular projection; a region in a stereographic or gnomonic projection.
- A horizon view: what is up now from a place. Altitude and azimuth in an azimuthal projection (zenith at the centre, horizon the rim), planets and the Moon marked, scrubbed through the night.
- Planetary orbits: the solar system from above, planets placed for a date, scrubbed through time. unicel's `solar_system.rs` already solves Kepler's equation by Newton's method for made-up orbits (its comment, line 70). glyphplot draws real ones.
- Light curves: flux against time for an exoplanet transit or a variable star, with gaps between observations, and phase-folded at the period.
- Ephemeris tables: rise, transit and set; RA, Dec, distance, magnitude by date. A table is text, so this is an example page, or the readout and accessibility table every chart has (agent 10).

What the sources offer:

| Source | What it is | Terms | Fit |
|---|---|---|---|
| HYG v4.1 (astronexus) | 119,626 rows (the Sun included) from Hipparcos, Yale BSC and Gliese. Columns include `ra` (hours), `dec`, `mag`, `ci`, `proper`, `con`, `x`,`y`,`z`, `var`. Epoch and equinox 2000.0. `hygdata_v41.csv` is 33,932,548 B. | CC BY-SA 4.0 (https://github.com/astronexus/HYG-Database/blob/main/hyg/CURRENT/LICENSE). The repo moved to https://codeberg.org/astronexus/hyg. | The sky chart's stars. A naked-eye subset is 118 KB gzipped. ShareAlike: the subset carries CC BY-SA 4.0, so it cannot sit inside an MIT crate. |
| Gaia DR3 (ESA) | 1.8 billion sources **(unverified)** | "Gaia data are distributed under the CC BY-NC 3.0 IGO license" (https://www.cosmos.esa.int/web/gaia-users/license) | No: NonCommercial. Not needed for a terminal-resolution sky either. |
| d3-celestial data | Constellation lines as GeoJSON (`constellations.lines.json`, 27,136 B), stars to magnitude 6 (`stars.6.json`, 656,721 B) | BSD-3-Clause, © 2015 Olaf Frohn (https://github.com/ofrohn/d3-celestial). The data readme names no upstream source for the lines. | Constellation lines for the sky example. Prefer HYG for stars, whose provenance is stated. |
| JPL approximate positions | Six Keplerian elements and their rates per century for 8 planets. Table 1 is fit for 1800–2050, Table 2 for 3000 BC–3000 AD. Stated errors run from about 15″ (Mercury) to 600″ (Saturn) in heliocentric longitude for 1800–2050 (https://ssd.jpl.nasa.gov/planets/approx_pos.html). | Not stated on the page. Orbital elements are facts. | The astronomy pack's planet model. 96 constants. |
| JPL Horizons API | Any body, any epoch, vectors or observer tables. One year of daily heliocentric vectors for one body is 366 rows, about 49.8 KB of JSON. | Not stated in the manual (first 100,000 characters read). JPL told Debian in 2022 that its DE ephemeris files are public domain (second-hand, https://metadata.ftp-master.debian.org/changelogs/main/s/swe-data/stable_copyright). Treat output as citeable facts; cite "NASA/JPL Horizons". | Precise tables; the Moon, comets and asteroids; a reference for tests. The API sends no CORS header (checked), so the site fetches at build time, not live. |
| Kepler and TESS light curves (MAST) | One TESS 2-minute light curve FITS: TIC 25155310, sector 1, 2,039,040 B, 20,076 rows, `TIME` in `BJD - 2457000, days`, `SAP_FLUX`, `PDCSAP_FLUX`. | After any exclusive period, "science data become available for public use without restriction" (https://archive.stsci.edu/publishing/data-use). | Light curve example. Teaches that astronomical time is a Julian date with an offset, not Unix seconds. |

Should the library compute positions?

| Option | Accuracy (measured 2026) | Cost | Covers |
|---|---|---|---|
| Expect positions; the page supplies RA/Dec or x/y/z (from Horizons or elsewhere) | as good as the source | nothing in wasm | everything, but every page must fetch or precompute, and Horizons cannot be called from a browser (no CORS) |
| Small Keplerian model (JPL Table 1) in the astronomy pack | Mars ≤ 35″, Jupiter ≤ 123″ geocentric; under 1/16 of a dot at whole-sky scale | 6,460 B gzip with RA/Dec and alt/az (probe) | 8 planets over 1800–2050 (Table 2 to 3000 BC–3000 AD). Not the Moon, comets or asteroids. |
| Truncated VSOP87 | about 1″ **(unverified)** | not measured; thousands of terms **(unverified)** | 8 planets, millennia |
| Full VSOP87 or JPL DE files | sub-arcsecond | megabytes **(unverified)** | everything |

Recommendation:

- Chart modules compute no astronomy. A sky chart takes RA and Dec (or alt and az) columns and a projection, and an orbit plot takes x and y in one unit with equal aspect.
- The astronomy pack computes planet positions from JPL Table 1 with Kepler's equation. It converts ecliptic to equatorial (obliquity 23.43928°) and equatorial to alt/az for a site and time (GMST, hour angle). It precesses J2000 catalogue positions to the date: about 50″ a year **(unverified)**, roughly 22′ by 2026, or 0.65 of a whole-sky dot, which grows with zoom. It also folds light curves.
- For the Moon, small bodies and precise ephemeris tables, the pack accepts Horizons output: a parser for the `$$SOE…$$EOE` CSV block.
- VSOP87 is not worth carrying: the cheaper model is already finer than a dot.
- This coordinates with agent 2, who also covers this. If agent 2 recommends that the library expect positions, the two agree on the chart side and differ only on whether a pack exists to compute them.

Library needs from astronomy:

- An angle scale that wraps (RA 0–24 h, azimuth 0–360°), with RA labelled in hours (`18h`) and Dec in signed degrees (`−23°`).
- Projections in the sky chart: equirectangular, stereographic and azimuthal.
- A magnitude scale (brighter is smaller, so marker size is inverse and log-like).
- A time scale that accepts Julian dates with an offset (BJD − 2457000) as well as Unix time. This is a unit in the data contract (agent 2): a timestamp column names its epoch and unit.
- Equal-aspect layout for orbits, which needs half the rows per column because a cell is about twice as tall as it is wide.

## Geoscience

- USGS earthquakes: the real-time GeoJSON feed `all_month.geojson` was 7,032,576 B (852,237 B gzipped) with 9,908 events on 2026-10-18. Each event has `mag`, `time` (ms), `place`, `depth` (the third coordinate), `tsunami` and `sig`. The feed sends CORS `*` and `cache-control: public, max-age=60` (checked). "USGS-authored or produced data and information are considered to be in the U.S. Public Domain"; credit "U.S. Geological Survey" (https://www.usgs.gov/information-policies-and-instructions/copyrights-and-credits).
- Charts: scatter of longitude against latitude, sized by magnitude and coloured by depth. Magnitude over time as a scatter. A log-count histogram by magnitude (Gutenberg–Richter). A live view polling every 60 s.
- Library needs: scatter with size and colour channels, a log y on histograms, and a plain longitude/latitude equirectangular projection. That projection is the sky chart's, with the axis flipped: RA grows to the left on a sky chart, longitude grows to the right.
- No pack: the GeoJSON is turned into columns in the example page in a few lines. A coastline outline is the one extra need, and it is data (Natural Earth is public domain **(unverified)**).
- Also public domain and live: NOAA CO-OPS tide gauges (water level against prediction, a line and a band) and NOAA SWPC Kp (a bar chart coloured by storm level).

## Other domains

- Biology and medicine: ECG strips (MIT-BIH Arrhythmia Database, PhysioNet, Open Data Commons Attribution License v1.0, https://physionet.org/content/mitdb/1.0.0/). A 360 Hz signal over 30 minutes **(unverified)** is a long line chart; it shows downsampling and scrubbing. Genome coverage tracks are area charts over an integer axis. No pack: WFDB parsing is a niche format.
- Sports: play-by-play and tracking data are rarely licensed for reuse. Name the chart shapes (shot charts are scatter, win probability is a line, pace is a step) and use synthetic data. No dataset is recommended.
- Operations dashboards: uptime bars, sparkline tables, gauges, status grids, Gantt and timelines. These are library chart types (agent 1) fed by the telemetry pack.
- Anomaly benchmarks: the Numenta Anomaly Benchmark (NAB) has 58 real and synthetic series **(unverified count)** with labelled anomaly windows, under MIT (https://github.com/numenta/NAB). `realAWSCloudwatch/ec2_cpu_utilization_825cc2.csv` has 4,032 rows at 5-minute steps. The labelled windows make it the natural fixture for annotation bands. Its Twitter-volume series should not be used: their origin is a third-party platform's data.

## Datasets

"Redistribution ok" means storing a copy in this public MIT repository (repo) and serving it from the public site (site). Sizes and SHA-256 were taken on 2026-10-18 from the file named.

| Domain | Dataset | Publisher | URL | Licence | Size | Redistribution ok? | Demonstrates |
|---|---|---|---|---|---|---|---|
| markets | Synthetic OHLCV and depth from a seed | glyphplot | — | MIT | generated | repo yes, site yes | candles, volume pane, depth, streaming at any rate, million-point scale |
| markets | WTI spot price, daily (`RWTC`) | U.S. EIA | https://www.eia.gov/dnav/pet/hist/LeafHandler.ashx?n=PET&s=RWTC&f=D (XLS 471,552 B); same series on FRED as `DCOILWTICO`, 10,640 rows, 175,886 B CSV | public domain, cite EIA (https://www.eia.gov/about/copyrights_reuse.php) | < 0.5 MB | repo yes (from EIA, not FRED), site yes | line, weekly candles aggregated from closes, session gaps, negative price on 2020-04-20 **(unverified date)** |
| markets | Daily par yield curve | U.S. Treasury | https://home.treasury.gov/resource-center/data-chart-center/interest-rates/daily-treasury-rates.csv/2026/all?type=daily_treasury_yield_curve&field_tdr_date_value=2026&page&_format=csv | public domain (US government work, 17 U.S.C. §105) | 2026: 16,229 B, 198 rows × 14 maturities, SHA-256 `b06702248327bb7aa652e3be1d104649bb7cce199bf04c176e4d62a9ff6803d8` | repo yes, site yes (CORS `*`) | heatmap (maturity × date), a curve scrubbed through time, percent formatter |
| markets | 10-year Treasury yield (`DGS10`), USD/EUR (`DEXUSEU`) | Federal Reserve (H.15, H.10) | https://www.federalreserve.gov/datadownload/ | public domain; FRED marks both "Public Domain: Citation Requested" | 269,075 B and 128,755 B as FRED CSV | repo yes (from the Fed), site yes | long line charts since 1962, zoom across decades |
| markets | Binance Vision klines | Binance | https://data.binance.vision | CC BY-NC-SA 4.0 plus Vision Dataset Terms | — | no, no | — (excluded) |
| markets | Coinbase Exchange feed | Coinbase | https://www.coinbase.com/legal/market_data | proprietary, display to third parties forbidden | — | no, no | — (excluded; adapter code may ship, data may not) |
| telemetry | NAB `realAWSCloudwatch`, `realKnownCause` | Numenta | https://github.com/numenta/NAB | MIT | 105–120 KB per series; `ec2_cpu_utilization_825cc2.csv` 4,032 rows, SHA-256 `d768419037c9db269343822957314f57ee21a7d9a4d41df2add0d1ba45ba84de` | repo yes, site yes | lines with gaps, anomaly windows as annotation bands, fixtures |
| telemetry | Wikimedia EventStreams `recentchange` | Wikimedia Foundation | https://stream.wikimedia.org/v2/stream/recentchange | event data licence not stated; counts are facts | live SSE | repo n/a, site yes (live, CORS `*`) | live counters to rates, live candles of edits per second, streaming append |
| telemetry | Synthetic Prometheus matrix and OTLP histograms | glyphplot | — | MIT | generated | yes, yes | rate, quantiles, latency heatmap, SLO band, counter resets |
| network | Synthetic flows and topology from a seed | glyphplot | — | MIT | generated | yes, yes | mirrored throughput, top talkers, graph |
| network | Wireshark sample captures | Wireshark wiki | https://wiki.wireshark.org/SampleCaptures | none stated | varies | no, no | — (user's own captures through the CLI instead) |
| network | RIPE RIS Live, RIPE Atlas | RIPE NCC | https://ris-live.ripe.net | RIPE NCC terms; redistribution restricted | — | no, no without permission | — |
| astronomy | HYG v4.1, mag ≤ 6.5 subset | astronexus (David Nash) | https://github.com/astronexus/HYG-Database (now https://codeberg.org/astronexus/hyg) | CC BY-SA 4.0 | full 33,932,548 B, SHA-256 `d9f69fd86bbf90a4e4d52b4c5c53eacfa6dfc0bfdef85bfd94f095e0bebe4ebd`; subset 8,920 stars, 305,537 B, 118,362 B gzip | repo only outside the MIT crates, under its own licence notice; site yes with attribution and the subset offered under CC BY-SA 4.0 | sky chart, magnitude scale, colour index |
| astronomy | Constellation lines | Olaf Frohn (d3-celestial) | https://github.com/ofrohn/d3-celestial/blob/master/data/constellations.lines.json | BSD-3-Clause | 27,136 B | yes with notice, yes | sky chart lines |
| astronomy | JPL approximate Keplerian elements, Tables 1 and 2 | NASA/JPL SSD | https://ssd.jpl.nasa.gov/planets/approx_pos.html | not stated; constants (facts) | 96 numbers (Table 1) | yes (cite), yes | orbit plot, planets on the sky chart, horizon view |
| astronomy | Horizons vectors, Mars, Earth, Jupiter, 2026 daily | NASA/JPL Horizons | `https://ssd.jpl.nasa.gov/api/horizons.api?format=json&COMMAND='499'&EPHEM_TYPE='VECTORS'&CENTER='500@10'&START_TIME='2026-01-01'&STOP_TIME='2027-01-01'&STEP_SIZE='1d'&VEC_TABLE='1'&CSV_FORMAT='YES'` | not stated; cite NASA/JPL Horizons | about 49.8 KB JSON each; SHA-256 Mars `0e415043c2f7040e7eb502d5c02dd0e9d762ff3688bc45e8aaf116369431c856`, Earth `c2017b5c32f1fb8133d4314db1d7387ff36d7f6b76619fb8e7d4cdd700694059`, Jupiter `22b3eb1fdeae4463693e066e1a0ba939a98c30a919cb6a57647d20ebee94d66b` (responses vary with retrieval time only in their header, **(unverified)**) | yes as test fixtures (cite) **(owner to confirm)**, yes | reference for the pack's tests, ephemeris table example |
| astronomy | TESS light curve TIC 25155310 sector 1 | NASA/MAST | https://archive.stsci.edu/missions/tess/tid/s0001/0000/0000/2515/5310/tess2018206045859-s0001-0000000025155310-0120-s_lc.fits | public use without restriction (https://archive.stsci.edu/publishing/data-use) | 2,039,040 B FITS, 20,076 rows, SHA-256 `12ddd623b04bdd354f27b5c89287b3a25a800d65cdf2733a1d0d04ab98e2d790` | yes as a < 1 MB CSV extract (time, flux) **(extract size unverified)**, yes | light curve, gaps between orbits, BJD time unit, phase folding |
| astronomy | Gaia DR3 | ESA | https://www.cosmos.esa.int/web/gaia-users/license | CC BY-NC 3.0 IGO | — | no, no | — (excluded) |
| geoscience | Earthquakes, past month | USGS | https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_month.geojson | public domain, credit USGS | 7,032,576 B (852,237 B gzip), 9,908 events on 2026-10-18 | repo: a < 1 MB extract only; site yes, live (CORS `*`) | scatter with size and colour, log histogram, live polling |
| geoscience | Planetary K-index; tide gauges | NOAA SWPC; NOAA CO-OPS | https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json; https://api.tidesandcurrents.noaa.gov/api/prod/ | public domain (US government work) | 4,330 B (Kp) | yes, yes (CORS `*`) | coloured bars, line against prediction band |
| biology | MIT-BIH Arrhythmia Database | PhysioNet | https://physionet.org/content/mitdb/1.0.0/ | Open Data Commons Attribution License v1.0 | not measured | yes with attribution, yes | long high-rate line, downsampling, scrubbing |

## Recommendation

1. **Where code goes.** Each need goes in one of three places:
   - The library: chart types, scales, formatters, annotations and data-contract units. These include the band chart (lower and upper), the step-area mode, panes sharing an axis, the index time scale, log scale, the wrapping angle scale, sky projections and equal aspect. The formatters cover SI, IEC bytes, bits per second, durations, price with tick size, signed percent, RA in hours and Dec in degrees. Time units cover Unix seconds and milliseconds, and Julian dates with an offset.
   - A domain pack: adapters into the data contract, domain arithmetic, presets.
   - An example page: transport (fetch, server-sent events, WebSocket), page layout, and dataset-specific glue under 50 lines.
2. **What a pack is.** A Rust crate `glyphplot-<domain>` that depends on the core's data-contract types and never on a chart or the grid. It is its own wasm module on the web (`packs/<domain>.wasm`) and linked natively in the CLI and the Rust crate. TypeScript only wires transport in examples.
3. **The first packs** (milestone v0.7.0, the "domain packs" phase):
   - **telemetry**: Prometheus matrix JSON, OTLP/JSON metrics, counter to rate, bucket quantiles, delta to cumulative, presets.
   - **markets**: trades to OHLCV, resampling, SMA, EMA, Bollinger, RSI, MACD, depth from order-book snapshots, synthetic market generator, presets.

   Then **astronomy** (v0.8.0): Keplerian planets, RA/Dec and alt/az, precession, Horizons parser, light-curve folding. Network (pcap, flows) and geoscience wait; their examples run on existing charts and synthetic or public-domain data.
4. **Library pieces the packs need, landing with the charts that use them:**
   - with candlestick, v0.2.0: index time scale and price formatter;
   - with line, v0.1.0: log scale and SI and duration formatters;
   - with the next tier, v0.6.0: band chart, step area and panes;
   - with sky and orbit charts, v0.8.0: angle scale and sky projections.
5. **The site's examples, in order:**
   - **live candles (synthetic)**: the owner's test, labelled "simulated market", with volume, SMA 20 and Bollinger 20/2, RSI below, scrubbing;
   - **live candles (real)**: Wikimedia edits per second, one candle a minute;
   - **service latency**: NAB and synthetic histograms; p50/p90/p99 on log y, a latency heatmap, an SLO band, an anomaly window;
   - **earthquakes now**: USGS live, scatter on longitude and latitude, magnitude histogram;
   - **yield curve**: Treasury heatmap and a scrubbed curve;
   - **oil since 1986**: EIA daily line with weekly candles;
   - **tonight's sky**: HYG subset, constellation lines, planets from the pack, horizon view for the reader's location (asked, never sent anywhere);
   - **the solar system**: orbit plot scrubbed through time;
   - **a transit**: TESS light curve, raw and phase-folded;
   - **network throughput**: synthetic, mirrored area and top talkers.
6. **Datasets policy.** A `datasets/manifest.json` names every dataset with publisher, URL, licence, a link to the terms, retrieval date, size and SHA-256. The repo commits only public-domain, CC0, CC BY, ODC-By, MIT or BSD data, and nothing over 1 MB. ShareAlike data (HYG) lives only under the site's data folder with its licence beside it, never in a crate. NonCommercial and proprietary data (Gaia, Binance, Coinbase, Kraken, FRED copyrighted series) is never committed and never served.

## Requirements to propose

Milestones assume one milestone per phase in the brief's order: v0.1.0 core and line; v0.2.0 contract, budgets, candlestick, bar, sparkline; v0.3.0 streaming; v0.4.0 site; v0.5.0 CLI; v0.6.0 next tier; v0.7.0 domain packs; v0.8.0 astronomy and long tail. Synthesis renumbers them if the phases differ.

Packs:

- REQ-PACK-010: A domain pack MUST be a crate under `crates/packs/<domain>/` that depends on the core's data-contract module and on no chart module, which `cargo tree -p glyphplot-<domain>` shows. (v0.7.0)
- REQ-PACK-020: A domain pack MUST build to its own wasm module, and no chart module's `.wasm` MAY contain a pack's exports, which `wasm-tools print` over each chart module shows. (v0.7.0)
- REQ-PACK-030: A pack's output MUST be columns in the data contract, accepted unchanged by the chart named in the pack's preset, which a test per preset shows by feeding the pack's output to that chart and comparing a snapshot. (v0.7.0)
- REQ-PACK-040: A pack's functions MUST be deterministic: the same input bytes give the same output bits natively and in wasm, which a test comparing native output with Node-run wasm output on every fixture shows. (v0.7.0)
- REQ-PACK-050: Each pack module MUST stay under its size line in `mise run size`, proposed at 32 KB gzipped (the measured probes are 1.5 KB and 6.5 KB). (v0.7.0)
- REQ-PACK-060: A pack's streaming functions MUST take amortised O(1) time per appended sample, which a benchmark appending 1,000,000 samples at window sizes 10 and 1,000 shows: time per sample differs by less than 2×. (v0.7.0)

Markets:

- REQ-MKT-010: The markets pack MUST compute SMA, EMA, Bollinger bands (window, k), RSI (Wilder) and MACD (fast, slow, signal) over a column and incrementally per sample. The two paths MUST agree to within 1e-9 relative error on every fixture. (v0.7.0)
- REQ-MKT-020: Indicator values MUST be NaN until the window is full, so a chart draws no line there. (v0.7.0)
- REQ-MKT-030: The markets pack MUST aggregate trades (time, price, size) into OHLCV of a given period aligned to a UTC origin. A test MUST check that open is the first trade, close the last, high and low the extremes, and volume the sum. (v0.7.0)
- REQ-MKT-040: The markets pack MUST generate synthetic OHLCV and order books from a seed, the same for the same seed, and every chart drawing them MUST show the text "simulated" in its title or legend. (v0.7.0)
- REQ-MKT-050: The library MUST provide an index time scale on which bars sit at consecutive indices whatever the time between them, with tick labels from the bars' timestamps. A fixture with a weekend gap MUST draw no empty columns. (v0.2.0)

Telemetry:

- REQ-TELE-010: The telemetry pack MUST read a Prometheus `query_range` matrix response, including `"NaN"`, `"+Inf"` and `"-Inf"` strings, into one column per series with labels as series names. (v0.7.0)
- REQ-TELE-020: The telemetry pack MUST turn a counter into a per-second rate that treats a drop in value as a reset, never as a negative rate. (v0.7.0)
- REQ-TELE-030: The telemetry pack MUST compute quantiles from classic `le` buckets, native histogram buckets and OTLP exponential histograms by linear interpolation within the bucket. A test MUST agree with Prometheus's documented `histogram_quantile` examples. (v0.7.0)
- REQ-TELE-040: A sample marked stale or "no recorded value" MUST become NaN in the column, so the line breaks. (v0.7.0)

Astronomy:

- REQ-ASTRO-010: The astronomy pack MUST compute the geocentric RA and Dec of the eight planets from JPL's approximate Keplerian elements. A test MUST keep them within 3′ of Horizons fixtures for every day of a committed year. (v0.8.0)
- REQ-ASTRO-020: The astronomy pack MUST convert RA and Dec to altitude and azimuth for a latitude, longitude and UTC time. A test MUST keep the results within 0.1° of a committed reference. (v0.8.0)
- REQ-ASTRO-030: The astronomy pack MUST precess J2000 positions to a given date. (v0.8.0)
- REQ-ASTRO-040: The astronomy pack MUST parse a Horizons `$$SOE … $$EOE` CSV block (vectors and observer tables) into columns. (v0.8.0)
- REQ-ASTRO-050: Chart modules MUST NOT compute celestial positions. A sky chart takes angle columns and a projection, which a grep for the pack's symbols in chart crates shows. (v0.8.0)
- REQ-ASTRO-060: The astronomy pack MUST fold a light curve at a period and epoch into phase −0.5 to 0.5. (v0.8.0)

Library pieces the domains need:

- REQ-FMT-010: The library MUST format bytes in IEC units (KiB, MiB), rates in SI bits per second (kb/s, Mb/s), durations from ns to days, and prices to a tick size. Each MUST be covered by a table test. (v0.2.0)
- REQ-FMT-020: The library MUST format right ascension in hours (`18h 54m`) and declination in signed degrees (`−23° 45′`). (v0.8.0)
- REQ-DATA-NNN (for agent 2's area): A timestamp column MUST declare its epoch and unit, including Unix seconds, Unix milliseconds and Julian date with an offset. A TESS fixture in `BJD − 2457000` days MUST draw with correct calendar ticks. (v0.6.0)

Datasets:

- REQ-DATASET-010: Every dataset committed or served by the site MUST have an entry in `datasets/manifest.json` with publisher, URL, licence name, licence URL, retrieval date, size and SHA-256. A check MUST fail when a file under `datasets/` or the site's data folder has no entry or a different hash. (v0.4.0)
- REQ-DATASET-020: No committed dataset MAY exceed 1 MB, or carry a NonCommercial, NoDerivatives or proprietary licence. A check MUST read the manifest's licence names against an allow-list. (v0.4.0)
- REQ-DATASET-030: ShareAlike datasets MUST NOT be included in any published crate or npm package. A check MUST list the package contents against the manifest. (v0.4.0)
- REQ-SITE-NNN: Every example over a live third-party feed MUST name the source and its licence on the page, and keep working (empty, with a message) when the feed is down. (v0.4.0)

## Decisions with a real alternative

**ADR (proposed): Domain packs are separate Rust crates and wasm modules.**

- Context: domains need adapters and arithmetic. The brief forbids bloating a chart module, and the CLI needs the same code.
- Options:
  - (a) Cargo features of chart modules.
  - (b) TypeScript beside the shell.
  - (c) A Rust crate per domain, its own wasm module, linked natively in the CLI.
- Recommendation: (c). One implementation serves web and terminal. Chart modules stay chart-only. The cost of crossing modules is one copy of new values, measured at 0.032 µs a value.

**ADR (proposed): Indicators live in the markets pack and are computed incrementally.**

- Options:
  - in the candlestick module;
  - in the markets pack;
  - in TypeScript.
- Recommendation: the markets pack (reasons under Markets).

**ADR (proposed): Celestial positions come from a Keplerian model in the astronomy pack, not from the library, and not from VSOP87.**

- Options:
  - the library computes;
  - the pack computes with JPL Table 1;
  - the pack computes with truncated VSOP87;
  - pages always supply positions.
- Recommendation: the pack with JPL Table 1, plus a Horizons parser. Measured error is under 1/16 of a dot at whole-sky scale for 6.5 KB gzipped.

**ADR (proposed): Session gaps are removed by an index time scale in the library, not by trading calendars.**

- Options:
  - a calendar-aware time scale fed by exchange calendars;
  - an index scale over the bars present.
- Recommendation: the index scale, as Lightweight Charts does. It needs no calendar data and works for any irregular series (a light curve's gaps, too). Calendars come later in the markets pack, only for marking expected but missing bars.

**ADR (proposed): The site's market demo runs on synthetic data, and a real live example uses a non-market public feed.**

- Options:
  - an exchange feed with written permission;
  - synthetic data;
  - candles of a public non-market feed.
- Recommendation: synthetic data for the owner's test, labelled as such, plus Wikimedia edits per second for a real feed. No exchange's terms checked allow display to third parties.

**ADR (proposed): A dataset manifest with a licence allow-list.**

- Options:
  - ad hoc data files;
  - a manifest with hashes and an allow-list checked in CI.
- Recommendation: the manifest (REQ-DATASET-010 to 030).

## Open questions for the owner

1. **Live market feed on the site.**
   - Options: (a) synthetic only; (b) synthetic plus Wikimedia candles; (c) ask Kraken (marketdata@kraken.com) or Coinbase for written permission to show a public feed.
   - Recommendation: (b) now, (c) only if a real market on the site matters enough to carry a revocable permission.
2. **Does the library compute celestial positions?** (Brief question 5.)
   - Options: (a) the library; (b) the astronomy pack (Keplerian); (c) pages supply positions.
   - Recommendation: (b), with the chart modules taking positions as (c) does.
3. **ShareAlike data on the site.**
   - Options: (a) commit the HYG subset under the site's data folder with a CC BY-SA 4.0 notice; (b) fetch it at site build time from codeberg and commit nothing; (c) use only BSD-licensed d3-celestial stars, whose upstream source is not stated.
   - Recommendation: (b), with the SHA-256 pinned in the manifest.
4. **The first domain pack.** (Brief question 7.)
   - Options: telemetry, markets, astronomy.
   - Recommendation: markets and telemetry together at v0.7.0, because the owner's test is a market chart and telemetry is the widest audience. Astronomy at v0.8.0 with the sky and orbit charts it needs.
5. **pcap support.**
   - Options: (a) a network pack that parses pcap and pcapng in wasm; (b) flows only, from CSV or IPFIX exports, parsed in the CLI; (c) leave captures to other tools.
   - Recommendation: (b) first, then (a) once its size is measured.
6. **Horizons output as committed test fixtures.**
   - Horizons states no licence. JPL calls its DE files public domain (second-hand).
   - Options: (a) commit about 150 KB of vectors with a citation; (b) fetch in a script and commit only hashes.
   - Recommendation: (a), since the tests must run offline.

## Sources

- The brief (`README.md`, "The briefs", topic 7) and the owner's decisions (`spec/adr/001` to `012`).
- `../animate-unicode/spec/ROADMAP.md` §6.2, REQ-DATA-004 (unicel's data shapes).
- `../animate-unicode/crates/unicel/src/pieces/data/candlesticks.rs` (seeded market, Mulberry32, after ascii.rest), `sparkline.rs`, `bar_chart.rs`.
- `../animate-unicode/crates/unicel/src/pieces/space/solar_system.rs` (Kepler's equation by Newton's method, line 70), `saptarishi.rs`, and the other space pieces' headers.
- Coinbase Market Data Terms of Use, updated 2026-08-07: https://www.coinbase.com/legal/market_data (read through a headless browser; plain fetch returned 403).
- Binance Vision Dataset Terms v1.0, 2026-08-26: https://data.binance.vision/Binance_Vision-Terms_of_Use.pdf (SHA-256 `0ec62cd26207f8be17f424d83240f671280ec4b17f7ef9a0026e7b6e0258121f`).
- Kraken API guide: https://docs.kraken.com/api/docs/guides/global-intro (via search result text); WebSocket v2: https://docs.kraken.com/websockets-v2.
- FRED legal: https://fred.stlouisfed.org/legal/; series pages for DCOILWTICO, DGS10, SP500 and DEXUSEU (copyright line read 2026-10-18).
- EIA copyright and reuse: https://www.eia.gov/about/copyrights_reuse.php.
- Treasury yield curve CSV (CORS checked): URL in Datasets.
- Lightweight Charts time scale: https://tradingview.github.io/lightweight-charts/docs/time-scale.
- Prometheus HTTP API: https://prometheus.io/docs/prometheus/latest/querying/api/; native histograms: https://prometheus.io/docs/specs/native_histograms/.
- OpenTelemetry metrics data model: https://opentelemetry.io/docs/specs/otel/metrics/data-model/.
- Wikimedia EventStreams: https://wikitech.wikimedia.org/wiki/Event_Platform/EventStreams_HTTP_Service.
- RIPE NCC: https://www.ripe.net/about-us/legal/ripe-data-repository-terms-and-conditions/, https://ripe.net/analyse/internet-measurements/routing-information-service-ris/commercial-use.
- Wireshark sample captures: https://wiki.wireshark.org/SampleCaptures.
- HYG: https://github.com/astronexus/HYG-Database (licence file `hyg/CURRENT/LICENSE`, CC BY-SA 4.0), https://codeberg.org/astronexus/hyg.
- Gaia licence: https://www.cosmos.esa.int/web/gaia-users/license.
- d3-celestial: https://github.com/ofrohn/d3-celestial (BSD-3-Clause, via the GitHub API).
- JPL approximate positions: https://ssd.jpl.nasa.gov/planets/approx_pos.html; Horizons manual: https://ssd.jpl.nasa.gov/horizons/manual.html; Horizons API: https://ssd-api.jpl.nasa.gov/doc/horizons.html; JPL on DE files (second-hand): https://metadata.ftp-master.debian.org/changelogs/main/s/swe-data/stable_copyright.
- MAST data use: https://archive.stsci.edu/publishing/data-use.
- USGS copyright: https://www.usgs.gov/information-policies-and-instructions/copyrights-and-credits; feed: https://earthquake.usgs.gov/earthquakes/feed/v1.0/geojson.php.
- NOAA SWPC and CO-OPS endpoints: URLs in Datasets (CORS checked).
- PhysioNet MIT-BIH: https://physionet.org/content/mitdb/1.0.0/.
- NAB: https://github.com/numenta/NAB (MIT, via the GitHub API and `LICENSE.txt`).
- Measurements: `discovery/bench/domains/` (README, `kepler_vs_horizons.py`, `sizes/`, `check.mjs`); downloads in the session scratchpad, not the repo.
