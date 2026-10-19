# discovery/bench/domains (throwaway)

Throwaway measurements for `discovery/domains.md`. Nothing here is library code, nothing here is
kept past discovery, and no dataset is stored here: the report records each dataset's URL, licence,
size and SHA-256 instead.

| File | What it measures | How to run |
|---|---|---|
| `kepler_vs_horizons.py` | Angular error of JPL's approximate Keplerian elements (Table 1) against Horizons vectors for Mars and Jupiter, daily through 2026 | Fetch `b399_2026.json`, `b499_2026.json`, `b599_2026.json` from the Horizons API (URL in the report), then `python3 -I kepler_vs_horizons.py <dir>` |
| `sizes/` | Wasm size of an astronomy pack (8 planets, RA/Dec, alt/az) and an indicator pack (SMA, EMA, Bollinger, RSI, MACD), each a Cargo feature, `no_std`, raw `extern "C"`, unicel's release profile, no wasm-bindgen, no wasm-opt | `cd sizes && mise exec -- cargo build --release --target wasm32-unknown-unknown --features astro,indicators`, then `gzip -9 -c <wasm> \| wc -c` |
| `check.mjs` | Calls the probe once (Mars RA/Dec) and times each indicator over 1,000,000 samples | `mise exec -- node check.mjs <domain_sizes.wasm>` |
