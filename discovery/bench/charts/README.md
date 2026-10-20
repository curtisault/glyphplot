# charts bench (throwaway)

This crate is throwaway discovery code for `discovery/charts.md`. It is not
part of glyphplot and nothing depends on it.

It times three ways of drawing one line series into a braille plot:
ratatui-widgets' `Chart` (one canvas line per pair of points), the same
per-segment idea without ratatui, and a per-dot-column min/max pass. It also
prints candle glyph samples. `-- time noise` reruns the timing with uniform noise
instead of a random walk, the worst case for drawing every segment.

Rerun, from the repository root:

```
mise exec -- cargo run --release --manifest-path discovery/bench/charts/Cargo.toml -- time
mise exec -- cargo run --release --manifest-path discovery/bench/charts/Cargo.toml -- time noise
mise exec -- cargo run --release --manifest-path discovery/bench/charts/Cargo.toml -- candles
```

Each configuration runs 5 times; the table prints the minimum and the median
in milliseconds and the number of non-blank cells drawn. Record `uptime`
before a run: the numbers in the report were taken on a shared machine.
