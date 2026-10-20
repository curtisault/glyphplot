"""Throwaway (see ../README.md): what chart text would add to a subset font.

Subsets Iosevka Fixed Extended 34.9.0 to a base set of ranges (ASCII, the
symbols a chart draws with: arrows, box drawing, geometric shapes, braille,
sextants, octants), then to the base plus each tier of chart-text ranges, and
prints for each the glyph count, woff2 bytes, advance widths, and which wanted
code points the source font lacks. Writes each subset into ./target/.

Its one argument is the path of the Iosevka release zip
PkgTTF-Unhinted-IosevkaFixed-34.9.0.zip, downloaded from Iosevka's GitHub
releases (https://github.com/be5invis/Iosevka/releases/tag/v34.9.0).

    uvx --from 'fonttools[woff]==4.60.1' python -I subset.py <zip>
"""
import io
import sys
import zipfile
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

# The ranges being measured, inclusive.
BASE = [(0x20,0x7E),(0xB0,0xB0),(0xB7,0xB7),(0x2022,0x2022),(0x2190,0x21FF),(0x2500,0x25FF),(0x2800,0x28FF),(0x1CD00,0x1CDE5),(0x1CEA0,0x1CEAF),(0x1FB00,0x1FBFF)]
# Tier A: what numbers, units and readouts need.
A = [(0xA0,0xBF),(0xD7,0xD7),(0xF7,0xF7),(0x2007,0x200A),(0x2010,0x2015),(0x2018,0x201F),(0x2026,0x2026),(0x202F,0x202F),
     (0x2030,0x2030),(0x2032,0x2033),(0x2070,0x209F),(0x20A0,0x20C0),(0x2116,0x2116),(0x2122,0x2122),(0x2212,0x2212),
     (0x2248,0x2248),(0x2260,0x2260),(0x2264,0x2265),(0x221E,0x221E),(0x0394,0x0394),(0x03A3,0x03A3),(0x03A9,0x03A9),(0x03BC,0x03BC),(0x03C3,0x03C3),(0x2713,0x2713),(0x2717,0x2717),(0x2605,0x2606),(0xFFFD,0xFFFD)]
# Tier B: Western European labels (Latin-1 letters, Latin Extended-A).
B = [(0xC0,0xFF),(0x100,0x17F)]
# Tier C: Greek and Cyrillic, and all mathematical operators.
C = [(0x370,0x3FF),(0x400,0x4FF),(0x2200,0x22FF)]

TIERS = [
    ("base", "base", BASE),
    ("+A numbers/units", "plusA", BASE + A),
    ("+A+B Latin", "plusA+B", BASE + A + B),
    ("+A+B+C Greek/Cyrillic/math", "plusA+B+C", BASE + A + B + C),
]
MEMBER = "IosevkaFixed-Extended.ttf"


def code_points(ranges):
    """Every code point of the inclusive ranges, in order."""
    out = []
    for lo, hi in ranges:
        out.extend(range(lo, hi + 1))
    return out


def options():
    """Unhinted woff2, all name records kept, no OpenType layout features,
    CFF desubroutinised, a drawn .notdef."""
    return subset.Options(
        flavor="woff2",
        hinting=False,
        desubroutinize=True,
        layout_features=[],
        name_IDs=["*"],
        notdef_outline=True,
    )


def write_subset(ttf, ranges, path):
    """Subsets the font bytes `ttf` to `ranges`, saves woff2 at `path`, and
    returns (glyphs in the cmap, bytes written, sorted advance widths)."""
    opts = options()
    font = TTFont(io.BytesIO(ttf))
    cutter = subset.Subsetter(options=opts)
    cutter.populate(unicodes=set(code_points(ranges)))
    cutter.subset(font)
    cmap = font.getBestCmap()
    hmtx = font["hmtx"]
    advances = sorted({hmtx[name][0] for name in set(cmap.values())})
    subset.save_font(font, str(path), opts)
    return len(cmap), path.stat().st_size, advances


def read_ttf(zip_path):
    with zipfile.ZipFile(zip_path) as archive:
        for name in archive.namelist():
            if name == MEMBER or name.endswith("/" + MEMBER):
                return archive.read(name)
    sys.exit(f"{zip_path}: no {MEMBER} inside")


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: subset.py <PkgTTF-Unhinted-IosevkaFixed-34.9.0.zip>")
    ttf = read_ttf(sys.argv[1])
    source = TTFont(io.BytesIO(ttf)).getBestCmap()
    out_dir = Path(__file__).parent / "target"
    out_dir.mkdir(exist_ok=True)
    for label, stem, ranges in TIERS:
        missing = [cp for cp in code_points(ranges) if cp not in source]
        glyphs, size, widths = write_subset(ttf, ranges, out_dir / f"{stem}.woff2")
        shown = " ".join(f"U+{cp:04X}" for cp in missing[:12])
        print(f"{label:30s} glyphs {glyphs:5d}  woff2 {size:6d} B  widths {widths}  source lacks {len(missing)}: {shown}")


if __name__ == "__main__":
    main()
