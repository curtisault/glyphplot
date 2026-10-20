# bench/surfaces (throwaway)

This crate is throwaway. It measured what `discovery/surfaces.md` reports and
is not part of glyphplot. Nothing builds from it; delete it when it is no
longer useful.

It holds one hello chart (`src/lib.rs`: a line of values drawn in braille into
a grid of code points and colour indices, a y axis of three labels, an x axis
in box drawing) and four ways to show it:

| Target | What |
|---|---|
| `hello-std` | std only: reads stdin, prints the frame as text. The floor for a CLI's size. |
| `hello-crossterm` | crossterm 0.29, as unicel's CLI: alternate screen, raw mode, mouse capture, a crosshair that follows the pointer, q or Esc to leave, its own 24-bit SGR writer. Piped, one frame as text. |
| `hello-ratatui` | ratatui 0.30 (`default-features = false`, `crossterm`) with ratatui's own `Chart` widget (braille marker, line graph), the same loop. |
| `hello-ratatui-grid` | ratatui 0.30 showing the hello chart's own grid through a `Widget` adapter, the same loop. |
| `pkg/` + `ssr.mjs` | the hello chart in wasm (`--features wasm`, wasm-pack `--target web`), rendered to text from Node: a server-side first frame. |

The release profile is unicel's (`opt-level = "s"`, LTO, one codegen unit,
`panic = "abort"`, `strip`).

## Rerun

From the repository root:

```sh
B=discovery/bench/surfaces
mise exec -- cargo build --release --manifest-path $B/Cargo.toml --bin hello-std
mise exec -- cargo build --release --manifest-path $B/Cargo.toml --features crossterm-cli --bin hello-crossterm
mise exec -- cargo build --release --manifest-path $B/Cargo.toml --features ratatui-cli --bin hello-ratatui --bin hello-ratatui-grid
for b in hello-std hello-crossterm hello-ratatui hello-ratatui-grid; do
  stat -f "%N %z" $B/target/release/$b; gzip -9c $B/target/release/$b | wc -c
done

mise exec -- wasm-pack build $B --release --target web --out-dir pkg --no-pack -- --features wasm
cd $B && for n in 1000 100000 1000000; do mise exec -- node ssr.mjs $n; done
```

Run a CLI in a terminal to try the mouse: `target/release/hello-crossterm`
(a demo series), or `seq 1 500 | awk '{print sin($1/20)}' > /tmp/s.txt` and
then `target/release/hello-crossterm /tmp/s.txt`. With no file and stdin not a
terminal, each reads stdin to its end first, so redirect `</dev/null` when
running one by hand without data.
