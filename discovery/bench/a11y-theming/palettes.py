"""Contrast and colour-vision-deficiency (CVD) separation of chart palettes.

Throwaway (see README.md). Run from the repository root:

    mise exec -- uv run --no-project --with colour-science==0.4.6 --with numpy==2.5.4 \
        --with matplotlib==3.10.0 python discovery/bench/a11y-theming/palettes.py

Method
- Contrast: WCAG 2.2 relative luminance and contrast ratio
  (https://www.w3.org/TR/WCAG22/#dfn-relative-luminance, #dfn-contrast-ratio).
- CVD: Machado, Oliveira and Fernandes 2009 matrices at severity 1.0
  (colour.blindness.matrix_cvd_Machado2009), applied to linear sRGB, clipped to [0, 1].
- Colour difference: CIEDE2000 on CIELAB (D65) of the simulated colours.
- "Fitted": a palette's hues and chromas kept in Oklab LCh and its lightness moved,
  by bisection, the least amount that gives 3:1 against the ground (WCAG 1.4.11),
  chroma reduced until the colour is in the sRGB gamut, checked after rounding to hex.
"""

import itertools
import sys

import colour
import numpy as np
from matplotlib import colormaps

VISIONS = {
    "normal": None,
    "deutan": colour.blindness.matrix_cvd_Machado2009("Deuteranomaly", 1.0),
    "protan": colour.blindness.matrix_cvd_Machado2009("Protanomaly", 1.0),
    "tritan": colour.blindness.matrix_cvd_Machado2009("Tritanomaly", 1.0),
}

GROUNDS = {
    # unicel's site: --bg dark, --bg light, --paper-bg (site/src/site.css lines 6, 13, 22)
    "dark #131518": "#131518",
    "light #f5f6f7": "#f5f6f7",
    "white #ffffff": "#ffffff",
    "paper #f4efe2": "#f4efe2",
}


def hex_to_rgb(h):
    h = h.lstrip("#")
    return np.array([int(h[i : i + 2], 16) / 255 for i in (0, 2, 4)])


def rgb_to_hex(rgb):
    rgb = np.clip(np.round(np.asarray(rgb) * 255), 0, 255).astype(int)
    return "#" + "".join(f"{c:02x}" for c in rgb)


def lin(rgb):
    rgb = np.asarray(rgb, dtype=float)
    return np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)


def luminance(rgb):
    return float(lin(rgb) @ np.array([0.2126, 0.7152, 0.0722]))


def contrast(a, b):
    la, lb = luminance(hex_to_rgb(a)), luminance(hex_to_rgb(b))
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def simulate(rgb, vision):
    m = VISIONS[vision]
    if m is None:
        return np.asarray(rgb, dtype=float)
    return colour.models.eotf_inverse_sRGB(np.clip(lin(rgb) @ m.T, 0, 1))


def lab(rgb):
    xyz = colour.sRGB_to_XYZ(np.asarray(rgb, dtype=float))
    return colour.XYZ_to_Lab(xyz, colour.CCS_ILLUMINANTS["CIE 1931 2 Degree Standard Observer"]["D65"])


def de(a, b, vision="normal"):
    return float(colour.delta_E(lab(simulate(hex_to_rgb(a), vision)), lab(simulate(hex_to_rgb(b), vision)), method="CIE 2000"))


def to_oklch(h):
    ok = colour.XYZ_to_Oklab(colour.sRGB_to_XYZ(hex_to_rgb(h)))
    return ok[0], float(np.hypot(ok[1], ok[2])), float(np.arctan2(ok[2], ok[1]))


def from_oklch(L, C, H):
    """Hex for an Oklab LCh colour, chroma reduced until in gamut."""
    for _ in range(60):
        ok = np.array([L, C * np.cos(H), C * np.sin(H)])
        rgb = colour.XYZ_to_sRGB(colour.Oklab_to_XYZ(ok))
        if np.all(rgb >= -1e-4) and np.all(rgb <= 1 + 1e-4):
            return rgb_to_hex(np.clip(rgb, 0, 1))
        C *= 0.95
    return rgb_to_hex(np.clip(rgb, 0, 1))


def fit(h, ground, target=3.0):
    """The colour nearest h in lightness with at least `target`:1 against ground."""
    if contrast(h, ground) >= target:
        return h
    L, C, H = to_oklch(h)
    dark_ground = luminance(hex_to_rgb(ground)) < 0.18
    lo, hi = (L, 1.0) if dark_ground else (0.0, L)
    for _ in range(40):
        mid = (lo + hi) / 2
        ok = contrast(from_oklch(mid, C, H), ground) >= target
        if dark_ground:
            lo, hi = (lo, mid) if ok else (mid, hi)
        else:
            lo, hi = (mid, hi) if ok else (lo, mid)
    out = from_oklch(hi if dark_ground else lo, C, H)
    step = 0.002 if dark_ground else -0.002
    L2 = hi if dark_ground else lo
    while contrast(out, ground) < target:
        L2 += step
        out = from_oklch(L2, C, H)
    return out


def ramp(name, n, lo=0.0, hi=1.0):
    cmap = colormaps[name]
    return [rgb_to_hex(cmap(x)[:3]) for x in np.linspace(lo, hi, n)]


def oklch_ramp(stops, n):
    """n colours along straight lines in Oklab LCh between (L, C, hue in degrees) stops."""
    out = []
    for x in np.linspace(0, 1, n):
        seg = min(int(x * (len(stops) - 1)), len(stops) - 2)
        f = x * (len(stops) - 1) - seg
        a, b = stops[seg], stops[seg + 1]
        ha = a[2] if a[1] > 0 else b[2]
        hb = b[2] if b[1] > 0 else a[2]
        out.append(from_oklch(a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, np.radians(ha + (hb - ha) * f)))
    return out


CATEGORICAL = {
    # Okabe and Ito 2008, https://jfly.uni-koeln.de/color/ (order: orange, sky blue, bluish green, yellow, blue, vermillion, reddish purple, black)
    "Okabe-Ito": ["#e69f00", "#56b4e9", "#009e73", "#f0e442", "#0072b2", "#d55e00", "#cc79a7", "#000000"],
    # d3-scale-chromatic 3.1.0 src/categorical/Tableau10.js
    "Tableau 10": ["#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7", "#9c755f", "#bab0ab"],
    # d3-scale-chromatic 3.1.0 src/categorical/observable10.js
    "Observable 10": ["#4269d0", "#efb118", "#ff725c", "#6cc5b0", "#3ca951", "#ff8ab7", "#a463f2", "#97bbf5", "#9c6b4e", "#9498a0"],
    # d3-scale-chromatic 3.1.0 src/categorical/Dark2.js (ColorBrewer)
    "ColorBrewer Dark2": ["#1b9e77", "#d95f02", "#7570b3", "#e7298a", "#66a61e", "#e6ab02", "#a6761d", "#666666"],
    # Paul Tol, https://personal.sron.nl/~pault/ "bright"
    "Tol bright": ["#4477aa", "#ee6677", "#228833", "#ccbb44", "#66ccee", "#aa3377", "#bbbbbb"],
}

# The proposal: Okabe-Ito's seven hues and a neutral, lightness fitted per ground.
OI7 = ["#e69f00", "#56b4e9", "#009e73", "#f0e442", "#0072b2", "#d55e00", "#cc79a7"]


def proposals():
    dark = [fit(c, "#131518") for c in OI7] + ["#a0a4ab"]
    light = [fit(c, "#f5f6f7") for c in OI7] + ["#6f747c"]
    # The light set's yellow, fitted to 3:1, lands near its orange; replace it with Okabe-Ito's black's role: a dark ink.
    return {"glyphplot dark (OI fitted)": dark, "glyphplot light (OI fitted)": light}


def pairs_report(name, pal):
    rows = []
    for v in VISIONS:
        d = [de(a, b, v) for a, b in itertools.combinations(pal, 2)]
        nb = [de(pal[i], pal[i + 1], v) for i in range(len(pal) - 1)]
        worst = min(itertools.combinations(range(len(pal)), 2), key=lambda p: de(pal[p[0]], pal[p[1]], v))
        rows.append((v, min(d), min(nb), worst))
    return rows


def categorical():
    print("## Categorical palettes\n")
    allp = dict(CATEGORICAL)
    allp.update(proposals())
    for name, pal in allp.items():
        print(f"### {name}\n")
        print("colours: " + " ".join(pal) + "\n")
        print("| ground | min contrast | colours under 3:1 |")
        print("|---|---|---|")
        for gname, g in GROUNDS.items():
            cs = [contrast(c, g) for c in pal]
            under = [f"{c} {r:.2f}" for c, r in zip(pal, cs) if r < 3.0]
            print(f"| {gname} | {min(cs):.2f} | {len(under)}: {', '.join(under) or '-'} |")
        print()
        print("| vision | min ΔE00 any pair | min ΔE00 neighbours | closest pair |")
        print("|---|---|---|---|")
        for v, mn, mnb, (i, j) in pairs_report(name, pal):
            print(f"| {v} | {mn:.1f} | {mnb:.1f} | {i + 1}–{j + 1} ({pal[i]} {pal[j]}) |")
        print()


def fitted_others():
    print("## Other palettes fitted to 3:1 by the same rule\n")
    print("| palette | ground | normal | deutan | protan | tritan |  (min ΔE00 any pair)")
    print("|---|---|---|---|---|---|")
    for name in ["Tableau 10", "Observable 10", "Okabe-Ito"]:
        src = CATEGORICAL[name]
        if name == "Okabe-Ito":
            src = src[:7]
        for gname, g in [("dark", "#131518"), ("light", "#f5f6f7")]:
            pal = [fit(c, g) for c in src]
            vals = [min(de(a, b, v) for a, b in itertools.combinations(pal, 2)) for v in VISIONS]
            print(f"| {name} fitted | {gname} | " + " | ".join(f"{x:.1f}" for x in vals) + " |  " + " ".join(pal))
    print()


def ordered(name, pal):
    Ls = {v: [lab(simulate(hex_to_rgb(c), v))[0] for c in pal] for v in VISIONS}
    mono = {v: all(np.diff(L) > 0) or all(np.diff(L) < 0) for v, L in Ls.items()}
    adj = {v: min(de(pal[i], pal[i + 1], v) for i in range(len(pal) - 1)) for v in VISIONS}
    ends = {g: (contrast(pal[0], gh), contrast(pal[-1], gh)) for g, gh in [("dark", "#131518"), ("light", "#f5f6f7")]}
    print(
        f"| {name} | "
        + " | ".join(f"{adj[v]:.1f}{'' if mono[v] else ' (L* not monotonic)'}" for v in VISIONS)
        + f" | {ends['dark'][0]:.2f} / {ends['dark'][1]:.2f} | {ends['light'][0]:.2f} / {ends['light'][1]:.2f} |"
    )
    return pal


def sequential_and_diverging():
    print("## Sequential and diverging scales, 9 steps\n")
    print("Adjacent-step min ΔE00 per vision; L* monotonic unless noted; contrast of first / last step against each ground.\n")
    print("| scale | normal | deutan | protan | tritan | dark ground first / last | light ground first / last |")
    print("|---|---|---|---|---|---|---|")
    scales = {
        "viridis": ramp("viridis", 9),
        "cividis": ramp("cividis", 9),
        "magma": ramp("magma", 9),
        "ColorBrewer Blues": ramp("Blues", 9),
        "viridis 0.25–1 (dark ground)": ramp("viridis", 9, 0.25, 1.0),
        "viridis 1–0 trimmed 0.85–0 (light ground)": ramp("viridis", 9, 0.85, 0.0),
        "cividis 0.25–1 (dark ground)": ramp("cividis", 9, 0.25, 1.0),
        "RdBu (ColorBrewer, light mid)": ramp("RdBu", 9),
        "PuOr (ColorBrewer, light mid)": ramp("PuOr", 9),
        "BrBG (ColorBrewer, light mid)": ramp("BrBG", 9),
        "glyphplot div dark (blue–dark–orange)": oklch_ramp([(0.82, 0.11, 245), (0.36, 0.0, 0), (0.82, 0.13, 65)], 9),
        "glyphplot div light (blue–light–orange)": oklch_ramp([(0.45, 0.14, 255), (0.96, 0.0, 0), (0.50, 0.14, 50)], 9),
    }
    for name, pal in scales.items():
        ordered(name, pal)
    print()
    # Diverging scales are two ramps: monotonic per half.
    print("Diverging halves (L* monotonic per half, by vision):\n")
    for name, pal in scales.items():
        if "div" not in name and "RdBu" not in name and "PuOr" not in name and "BrBG" not in name:
            continue
        half = len(pal) // 2
        res = []
        for v in VISIONS:
            L = [lab(simulate(hex_to_rgb(c), v))[0] for c in pal]
            a, b = np.diff(L[: half + 1]), np.diff(L[half:])
            res.append(f"{v}: {'yes' if (all(a > 0) or all(a < 0)) and (all(b > 0) or all(b < 0)) else 'no'}")
        # end-to-end separation: the two ends must differ under every vision
        ends = [f"{v} ends ΔE00 {de(pal[0], pal[-1], v):.1f}" for v in VISIONS]
        print(f"- {name}: {' '.join(pal)}; " + ", ".join(res) + "; " + ", ".join(ends))
    print()


def text_tokens():
    print("## Theme text and rule tokens against their grounds\n")
    print("| token | colour | ground | contrast |")
    print("|---|---|---|---|")
    for tok, c, g in [
        ("ink (dark)", "#e8ebef", "#131518"),
        ("muted (dark)", "#8c95a1", "#131518"),
        ("rule (dark)", "#262a2f", "#131518"),
        ("ink (light)", "#1e2125", "#f5f6f7"),
        ("muted (light)", "#5f6873", "#f5f6f7"),
        ("rule (light)", "#dfe2e6", "#f5f6f7"),
        ("paper ink", "#2a2622", "#f4efe2"),
        ("neutral series (dark)", "#a0a4ab", "#131518"),
        ("neutral series (light)", "#6f747c", "#f5f6f7"),
    ]:
        print(f"| {tok} | {c} | {g} | {contrast(c, g):.2f} |")
    print()


if __name__ == "__main__":
    print("Run on", sys.platform, "colour-science", colour.__version__, "\n")
    text_tokens()
    categorical()
    fitted_others()
    sequential_and_diverging()
