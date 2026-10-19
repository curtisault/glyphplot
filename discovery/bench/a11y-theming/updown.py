"""Up/down (candle) colour pairs: contrast against the grounds and CIEDE2000 between the
two under each simulated vision. Throwaway (see README.md). Run as palettes.py, with updown.py."""
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from palettes import VISIONS, contrast, de  # noqa: E402

PAIRS = {
    # crates/unicel/src/pieces/data/candlesticks.rs:26 (palette[0], palette[1]) on its ground #0c0e12
    "unicel candlesticks green/red": ("#4ac87a", "#e05a5a"),
    "TradingView-like green/red": ("#26a69a", "#ef5350"),
    "Okabe-Ito bluish green/vermillion": ("#009e73", "#d55e00"),
    "blue/orange (OI sky blue/orange)": ("#56b4e9", "#e69f00"),
}
print("| pair | up | down | " + " | ".join(f"ΔE00 {v}" for v in VISIONS) + " | contrast on #131518 (up/down) | on #f5f6f7 (up/down) |")
print("|---|---|---|" + "---|" * len(VISIONS) + "---|---|")
for name, (u, d) in PAIRS.items():
    print(
        f"| {name} | {u} | {d} | "
        + " | ".join(f"{de(u, d, v):.1f}" for v in VISIONS)
        + f" | {contrast(u, '#131518'):.2f} / {contrast(d, '#131518'):.2f} | {contrast(u, '#f5f6f7'):.2f} / {contrast(d, '#f5f6f7'):.2f} |"
    )
