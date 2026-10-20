"""The search of optimise.py, held near Okabe-Ito: each colour's hue within ±25° of its
Okabe-Ito seed (Oklab), chroma at least 0.06 (the eighth a neutral, chroma under 0.03),
and Oklab lightness inside a band per ground, so no series stands out by weight alone.
Throwaway (see README.md). Run as optimise.py, with anchored.py.
"""

import sys

import colour
import numpy as np

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from optimise import fit_all, report, score  # noqa: E402
from palettes import from_oklch, hex_to_rgb  # noqa: E402

SEEDS = ["#e69f00", "#56b4e9", "#009e73", "#f0e442", "#0072b2", "#d55e00", "#cc79a7"]


def oklch(h):
    ok = colour.XYZ_to_Oklab(colour.sRGB_to_XYZ(hex_to_rgb(h)))
    return ok[0], float(np.hypot(ok[1], ok[2])), float(np.arctan2(ok[2], ok[1]))


def ok_slot(h, k, seeds, band):
    L, C, H = oklch(h)
    if not (band[0] <= L <= band[1]):
        return False
    if k == 7:
        return C < 0.03
    dh = (np.degrees(H - seeds[k][2]) + 180) % 360 - 180
    return C >= 0.06 and abs(dh) <= 25


def search(grounds, band, iters=6000, restarts=5, seed=2):
    rng = np.random.default_rng(seed)
    seeds = [oklch(h) for h in SEEDS]
    best, best_s = None, -1
    for _ in range(restarts):
        pal = []
        for k in range(8):
            for _try in range(200):
                if k == 7:
                    cand = from_oklch(rng.uniform(*band), 0.01, 0.0)
                else:
                    cand = from_oklch(rng.uniform(*band), rng.uniform(0.08, 0.17), seeds[k][2] + np.radians(rng.normal(0, 8)))
                cand = fit_all(cand, grounds)
                if cand and ok_slot(cand, k, seeds, band):
                    break
            pal.append(cand)
        s = score(pal)
        for _ in range(iters):
            k = int(rng.integers(8))
            L, C, H = oklch(pal[k])
            cand = fit_all(from_oklch(L + rng.normal(0, 0.03), max(0.0, C + rng.normal(0, 0.02)), H + rng.normal(0, 0.15)), grounds)
            if not cand or not ok_slot(cand, k, seeds, band):
                continue
            trial = pal[:k] + [cand] + pal[k + 1 :]
            ts = score(trial)
            if ts >= s:
                pal, s = trial, ts
        if s > best_s:
            best, best_s = pal, s
    return best, best_s


if __name__ == "__main__":
    for label, grounds, band in (
        ("dark", ["#131518", "#1a1d21"], (0.62, 0.90)),
        ("light", ["#f4efe2", "#f5f6f7", "#ffffff"], (0.38, 0.64)),
    ):
        pal, s = search(grounds, band)
        report(f"anchored to Okabe-Ito, {label} grounds {', '.join(grounds)}, Oklab L {band} (objective {s:.1f})", pal, grounds)
