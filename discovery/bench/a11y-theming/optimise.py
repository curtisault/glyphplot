"""Search for a categorical palette that keeps 3:1 against its grounds and the widest
smallest CIEDE2000 difference between any two colours under normal vision and the
three simulated dichromacies (Machado 2009, severity 1). Throwaway (see README.md).

    mise exec -- uv run --no-project --with colour-science==0.4.6 --with numpy==2.5.4 \
        --with matplotlib==3.10.0 python discovery/bench/a11y-theming/optimise.py

Seeded from Okabe-Ito's hues; a fixed random seed per run, so the output repeats.
Hill climbing with restarts: perturb one colour's Oklab L, C, h; refit its lightness to
the contrast floor; keep the change if the objective does not fall.
"""

import itertools
import sys

import colour
import numpy as np

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from palettes import VISIONS, contrast, de, fit, from_oklch, hex_to_rgb, lin  # noqa: E402

D65 = colour.CCS_ILLUMINANTS["CIE 1931 2 Degree Standard Observer"]["D65"]
MATS = [np.eye(3)] + [m for m in VISIONS.values() if m is not None]


def labs(hexes):
    rgb = np.array([hex_to_rgb(h) for h in hexes])
    out = []
    for m in MATS:
        sim = colour.models.eotf_inverse_sRGB(np.clip(lin(rgb) @ m.T, 0, 1))
        out.append(colour.XYZ_to_Lab(colour.sRGB_to_XYZ(sim), D65))
    return np.array(out)  # vision x n x 3


def score(hexes):
    L = labs(hexes)
    i, j = np.array(list(itertools.combinations(range(len(hexes)), 2))).T
    d = colour.delta_E(L[:, i], L[:, j], method="CIE 2000")
    return float(d.min())


def meets(h, grounds):
    return all(contrast(h, g) >= 3.0 for g in grounds)


def fit_all(h, grounds):
    for g in grounds:
        h = fit(h, g)
    return h if meets(h, grounds) else None


def search(n, grounds, seed_hues, iters=4000, restarts=6, rng_seed=1):
    rng = np.random.default_rng(rng_seed)
    dark = sum(hex_to_rgb(grounds[0])) < 1.5
    best_all, best_s = None, -1
    for r in range(restarts):
        hues = list(seed_hues[:n]) + list(rng.uniform(0, 360, max(0, n - len(seed_hues))))
        pal = []
        for k in range(n):
            L = rng.uniform(0.72, 0.88) if dark else rng.uniform(0.42, 0.6)
            C = rng.uniform(0.06, 0.17)
            pal.append(fit_all(from_oklch(L, C, np.radians(hues[k] + rng.normal(0, 8))), grounds) or "#808080")
        s = score(pal)
        for _ in range(iters):
            k = rng.integers(n)
            ok = colour.XYZ_to_Oklab(colour.sRGB_to_XYZ(hex_to_rgb(pal[k])))
            L, C, H = ok[0], np.hypot(ok[1], ok[2]), np.arctan2(ok[2], ok[1])
            cand = from_oklch(
                float(np.clip(L + rng.normal(0, 0.04), 0.25, 0.97)),
                float(np.clip(C + rng.normal(0, 0.02), 0.0, 0.3)),
                H + rng.normal(0, 0.25),
            )
            cand = fit_all(cand, grounds)
            if cand is None:
                continue
            trial = pal[:k] + [cand] + pal[k + 1 :]
            ts = score(trial)
            if ts >= s:
                pal, s = trial, ts
        if s > best_s:
            best_all, best_s = pal, s
    # order: greedy chain so neighbours differ most under the worst vision
    order = [0]
    left = set(range(1, n))
    while left:
        last = best_all[order[-1]]
        nxt = max(left, key=lambda j: min(de(last, best_all[j], v) for v in VISIONS))
        order.append(nxt)
        left.remove(nxt)
    return [best_all[i] for i in order], best_s


def report(name, pal, grounds):
    print(f"### {name}\n")
    print("colours: " + " ".join(pal) + "\n")
    print("contrast: " + ", ".join(f"{g} min {min(contrast(c, g) for c in pal):.2f}" for g in grounds))
    for v in VISIONS:
        d = min(de(a, b, v) for a, b in itertools.combinations(pal, 2))
        nb = min(de(pal[i], pal[i + 1], v) for i in range(len(pal) - 1))
        print(f"- {v}: min ΔE00 any pair {d:.1f}, neighbours {nb:.1f}")
    print()


if __name__ == "__main__":
    oi_hues = []
    for h in ["#e69f00", "#56b4e9", "#009e73", "#f0e442", "#0072b2", "#d55e00", "#cc79a7"]:
        ok = colour.XYZ_to_Oklab(colour.sRGB_to_XYZ(hex_to_rgb(h)))
        oi_hues.append(float(np.degrees(np.arctan2(ok[2], ok[1]))))
    oi_hues.append(0.0)
    dark_g = ["#131518", "#1a1d21"]
    light_g = ["#f4efe2", "#f5f6f7", "#ffffff"]
    for n in (8, 10):
        for label, grounds in (("dark", dark_g), ("light", light_g)):
            pal, s = search(n, grounds, oi_hues + [200.0, 300.0])
            report(f"searched {n}, {label} grounds {', '.join(grounds)} (objective {s:.1f})", pal, grounds)
