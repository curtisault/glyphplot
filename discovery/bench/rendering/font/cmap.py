# Throwaway (see ../README.md): what chart text needs, against a font's cmap.  uvx --from 'fonttools[woff]==4.60.1' python -I cmap.py <font>
import sys
from fontTools.ttLib import TTFont
f = TTFont(sys.argv[1])
cmap = f.getBestCmap()
print("glyphs in cmap:", len(cmap))
want = {
 "digits 0-9": range(0x30,0x3A), "A-Z": range(0x41,0x5B), "a-z": range(0x61,0x7B),
 "hyphen-minus -": [0x2D], "minus U+2212": [0x2212], "degree U+00B0": [0xB0], "micro U+00B5": [0xB5], "mu U+03BC": [0x3BC],
 "plus-minus U+00B1": [0xB1], "multiply U+00D7": [0xD7], "divide U+00F7": [0xF7],
 "middle dot U+00B7": [0xB7], "bullet U+2022": [0x2022], "ellipsis U+2026": [0x2026],
 "thin space U+2009": [0x2009], "narrow nbsp U+202F": [0x202F], "nbsp U+00A0": [0xA0], "figure space U+2007": [0x2007],
 "en dash U+2013": [0x2013], "em dash U+2014": [0x2014],
 "euro U+20AC": [0x20AC], "pound U+00A3": [0xA3], "yen U+00A5": [0xA5], "cent U+00A2": [0xA2], "rupee U+20B9": [0x20B9], "bitcoin U+20BF": [0x20BF], "won U+20A9":[0x20A9],
 "dollar $": [0x24], "percent %": [0x25], "per mille U+2030": [0x2030],
 "superscript 2/3 U+00B2/B3": [0xB2,0xB3], "superscript 1 U+00B9": [0xB9], "superscripts U+2070-209F": list(range(0x2070,0x207A)),
 "prime/double prime U+2032/2033": [0x2032,0x2033], "Delta U+0394": [0x394], "Sigma U+03A3": [0x3A3], "sigma U+03C3": [0x3C3], "Omega U+03A9": [0x3A9], "alpha..omega U+03B1-03C9": list(range(0x3B1,0x3CA)),
 "infinity U+221E": [0x221E], "approx U+2248": [0x2248], "<= >= U+2264/2265": [0x2264,0x2265], "not equal U+2260": [0x2260],
 "arrows U+2190-2193": list(range(0x2190,0x2194)), "triangles ▲▼ U+25B2/25BC": [0x25B2,0x25BC], "small triangles ▴▾ U+25B4/25BE":[0x25B4,0x25BE],
 "circle ● U+25CF": [0x25CF], "box drawing U+2500-257F": list(range(0x2500,0x2580)),
 "Latin-1 letters U+00C0-00FF": [c for c in range(0xC0,0x100) if c not in (0xD7,0xF7)],
 "Latin Ext-A U+0100-017F": list(range(0x100,0x180)),
 "fraction slash U+2044": [0x2044], "vulgar ½ U+00BD": [0xBD], "check ✓ U+2713": [0x2713], "cross ✗ U+2717":[0x2717],
 "black/white square ■□": [0x25A0,0x25A1], "diamond ◆ U+25C6": [0x25C6], "star ★ U+2605":[0x2605],
 "replacement U+FFFD":[0xFFFD], "zero-width/LRM U+200E":[0x200E],
}
for k, cps in want.items():
    cps = list(cps); have = [c for c in cps if c in cmap]
    print(f"{k:34s} {len(have)}/{len(cps)}" + ("" if len(have)==len(cps) else "  missing: " + " ".join(f"U+{c:04X}" for c in cps if c not in cmap)[:120]))
# ranges summary
cs = sorted(cmap); runs=[]; s=p=cs[0]
for c in cs[1:]:
    if c==p+1: p=c; continue
    runs.append((s,p)); s=p=c
runs.append((s,p))
print("runs:", ", ".join(f"U+{a:04X}" + ("" if a==b else f"-{b:04X}") for a,b in runs))
print("unitsPerEm", f["head"].unitsPerEm, "ascender", f["hhea"].ascent, "descender", f["hhea"].descent, "xHeight", getattr(f["OS/2"],"sxHeight",None), "capHeight", getattr(f["OS/2"],"sCapHeight",None))
