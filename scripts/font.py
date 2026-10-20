"""Builds web/fonts/glyphplot-mono.woff2, the bundled font (REQ-FONT-010, ADR-121).

The font is a subset of Iosevka Fixed Extended 34.9.0, Regular (SIL OFL 1.1),
renamed to the family "glyphplot mono": the base ranges as Iosevka has them,
plus tiers A and B of the plan's font section. The script

1. downloads Iosevka's release zip and its licence once into target/font/ and
   checks each against a SHA-256 pinned below;
2. extracts the Fixed Extended Regular face, subsets it, draws its composite
   glyphs whole and renames it;
3. checks the woff2 it wrote, and fails on any glyph not 600 units wide, any
   tier A code point missing other than the eight Iosevka lacks, any tier B
   code point missing, a kept name record still naming Iosevka, or more than
   32,768 bytes;
4. writes web/fonts/glyphplot-mono.woff2, web/fonts/unicode-range.txt (the
   CSS unicode-range, from the font's own cmap) and web/fonts/OFL.txt.

With --check it builds into a temporary file and fails when the woff2, the
unicode-range or the licence differ byte for byte from the checked-in files.

    mise exec -- uv run --no-project --with 'fonttools[woff]==4.60.1' --with 'brotli==1.2.0' \\
        python -I scripts/font.py [--check]

fontTools and brotli are pinned in that command and checked below: brotli
writes the woff2's bytes, so another version may give another file.
"""

import hashlib
import io
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

import brotli
import fontTools
from fontTools import subset
from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "target" / "font"
OUT = ROOT / "web" / "fonts"
WOFF2 = OUT / "glyphplot-mono.woff2"
RANGE = OUT / "unicode-range.txt"
LICENCE = OUT / "OFL.txt"

FONTTOOLS_VERSION = "4.60.1"
BROTLI_VERSION = "1.2.0"

RELEASE = "https://github.com/be5invis/Iosevka/releases/download/v34.9.0/"
ZIP_NAME = "PkgTTF-Unhinted-IosevkaFixed-34.9.0.zip"
# Pinned from the first download (2026-10-18, `shasum -a 256`); it matches the
# line for this zip in the release's own SHA-256.txt.
ZIP_SHA256 = "d89f5705b5be44572b83e3f482ac9575383effedd605e838f919ce91f031f56a"
# The zip holds only the faces; the licence is LICENSE.md at the release's tag.
# Pinned from the first download (2026-10-18, `shasum -a 256`).
LICENCE_URL = "https://raw.githubusercontent.com/be5invis/Iosevka/v34.9.0/LICENSE.md"
LICENCE_SHA256 = "4ba53c7c1cb39279aae5f8d7d22054c485c71169920e5a36ed098b115e2e3c5d"
MEMBER = "IosevkaFixed-Extended.ttf"

FAMILY = "glyphplot mono"
UNITS = 600
MAX_BYTES = 32_768

# The base ranges, inclusive, taken as Iosevka has them (REQ-FONT-010): ASCII,
# degree, middle dot, bullet, arrows, box drawing, blocks, geometric shapes,
# braille, octants, sextants and the rest of legacy computing.
BASE = [
    (0x0020, 0x007E),
    (0x00B0, 0x00B0),
    (0x00B7, 0x00B7),
    (0x2022, 0x2022),
    (0x2190, 0x21FF),
    (0x2500, 0x25FF),
    (0x2800, 0x28FF),
    (0x1CD00, 0x1CDE5),
    (0x1CEA0, 0x1CEAF),
    (0x1FB00, 0x1FBFF),
]
# These must be there in full: the base without them is no chart font.
BASE_REQUIRED = [(0x0020, 0x007E), (0x00B0, 0x00B0), (0x00B7, 0x00B7), (0x2022, 0x2022)]

# Tier A, inclusive: numbers, units, currency and marks (REQ-FONT-010, the
# plan's font section). "The currency signs" is U+20A0-20C0.
TIER_A = [
    (0x00A0, 0x00BF),
    (0x00D7, 0x00D7),
    (0x00F7, 0x00F7),
    (0x2007, 0x200A),
    (0x2010, 0x2015),
    (0x2018, 0x201F),
    (0x2026, 0x2026),
    (0x202F, 0x202F),
    (0x2030, 0x2030),
    (0x2032, 0x2033),
    (0x2070, 0x209F),
    (0x20A0, 0x20C0),
    (0x2116, 0x2116),
    (0x2122, 0x2122),
    (0x2212, 0x2212),
    (0x221E, 0x221E),
    (0x2248, 0x2248),
    (0x2260, 0x2260),
    (0x2264, 0x2265),
    (0x0394, 0x0394),  # Δ
    (0x03A3, 0x03A3),  # Σ
    (0x03A9, 0x03A9),  # Ω
    (0x03BC, 0x03BC),  # μ (µ U+00B5 is in U+00A0-00BF)
    (0x03C3, 0x03C3),  # σ
    (0x2713, 0x2713),
    (0x2717, 0x2717),
    (0x2605, 0x2606),
    (0xFFFD, 0xFFFD),
]
# The tier A code points Iosevka 34.9.0 lacks, read from its cmap: these and no
# others may be missing. U+2072 and 2073 are unassigned; the six others are
# rare currency signs (euro-currency, cruzeiro, pfennig, livre tournois,
# Spesmilo, Nordic mark).
TIER_A_ABSENT = {0x2072, 0x2073, 0x20A0, 0x20A2, 0x20B0, 0x20B6, 0x20B7, 0x20BB}

# Tier B, inclusive: Western European letters, Latin-1's letters and Latin
# Extended-A (REQ-FONT-020). Iosevka 34.9.0 has every one, read from its cmap.
TIER_B = [
    (0x00C0, 0x00FF),
    (0x0100, 0x017F),
]

# The name records kept, and what the renamed ones say. 0 (copyright), 2, 5, 8,
# 9, 13 and 14 (designer, licence) keep Iosevka's text; 10 (Iosevka's
# description), 16, 17, 21, 22 (typographic and WWS names, the same as 1 and 2
# here) and 256 and up (stylistic set names, gone with the layout tables) go.
NAME_IDS = [0, 1, 2, 3, 4, 5, 6, 8, 9, 13, 14]
RENAMED = {
    1: FAMILY,
    3: f"{FAMILY} 34.9.0",
    4: FAMILY,
    6: "glyphplot-mono",
}


def fail(message):
    """Stops the build with `message`."""
    sys.exit(f"font.py: {message}")


def code_points(ranges):
    """Every code point of the inclusive `ranges`, as a set."""
    return {cp for lo, hi in ranges for cp in range(lo, hi + 1)}


def u(cp):
    """`cp` written as U+XXXX."""
    return f"U+{cp:04X}"


def fetch(url, sha256, path):
    """The bytes at `url`, downloaded once to `path` and checked against `sha256`."""
    if not path.exists():
        print(f"downloading {url}")
        path.parent.mkdir(parents=True, exist_ok=True)
        part = path.with_name(path.name + ".part")
        with urllib.request.urlopen(url) as response, open(part, "wb") as out:
            while chunk := response.read(1 << 20):
                out.write(chunk)
        part.replace(path)
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != sha256:
        fail(f"{path} has SHA-256 {digest}, not the pinned {sha256}; delete it to download it again")
    return data


def options():
    """Unhinted woff2 with no OpenType layout tables, the kept name records, no
    glyph names and the source's timestamps, so that the bytes are the same
    from one build to the next."""
    opts = subset.Options(
        flavor="woff2",
        hinting=False,
        desubroutinize=True,
        layout_features=[],
        name_IDs=NAME_IDS,
        name_languages=["*"],
        notdef_outline=True,
        glyph_names=False,
        recalc_timestamp=False,
    )
    opts.drop_tables += ["GDEF", "GPOS", "GSUB", "DSIG"]
    return opts


def decompose(font):
    """Draws every composite glyph of `font` as a simple one. Iosevka builds
    some symbols (U+00A9, U+00BC, U+2122, struck arrows and currencies) from
    unencoded zero-width parts; drawn whole, they need no part, and every glyph
    left is 600 units wide."""
    glyf = font["glyf"]
    glyphs = font.getGlyphSet()
    for name in font.getGlyphOrder():
        if glyf[name].isComposite():
            recording = DecomposingRecordingPen(glyphs)
            glyphs[name].draw(recording)
            pen = TTGlyphPen(None)
            recording.replay(pen)
            glyf[name] = pen.glyph()


def build(ttf):
    """The woff2 bytes of the subset of `ttf`, the source face's bytes."""
    opts = options()
    unicodes = code_points(BASE) | code_points(TIER_A) | code_points(TIER_B)
    font = TTFont(io.BytesIO(ttf), recalcTimestamp=False, recalcBBoxes=True)
    for _ in range(2):
        # The second pass drops the parts the first pass's composites used.
        cutter = subset.Subsetter(options=opts)
        cutter.populate(unicodes=unicodes)
        cutter.subset(font)
        decompose(font)
    for record in font["name"].names:
        if record.nameID in RENAMED:
            record.string = RENAMED[record.nameID]
    out = io.BytesIO()
    subset.save_font(font, out, opts)
    return out.getvalue()


def check(woff2, source_cmap):
    """Fails on any rule of REQ-FONT-010 the woff2 breaks; returns its cmap."""
    font = TTFont(io.BytesIO(woff2))
    if font.flavor != "woff2":
        fail(f"the output is {font.flavor}, not woff2")

    hmtx = font["hmtx"]
    wrong = [(name, hmtx[name][0]) for name in font.getGlyphOrder() if hmtx[name][0] != UNITS]
    if wrong:
        fail(f"{len(wrong)} glyphs are not {UNITS} units wide: {wrong[:20]}")

    cmap = font.getBestCmap()
    absent = {cp for cp in code_points(TIER_A) if cp not in source_cmap}
    if absent != TIER_A_ABSENT:
        lacking = " ".join(u(cp) for cp in sorted(absent - TIER_A_ABSENT))
        found = " ".join(u(cp) for cp in sorted(TIER_A_ABSENT - absent))
        fail(f"the source lacks tier A code points not listed in TIER_A_ABSENT ({lacking or 'none'}), or has listed ones ({found or 'none'})")
    gaps = sorted((code_points(TIER_A) - TIER_A_ABSENT) - cmap.keys())
    if gaps:
        fail(f"the font lacks tier A code points: {' '.join(u(cp) for cp in gaps)}")
    gaps = sorted(code_points(TIER_B) - cmap.keys())
    if gaps:
        fail(f"the font lacks tier B code points: {' '.join(u(cp) for cp in gaps)}")
    gaps = sorted(code_points(BASE_REQUIRED) - cmap.keys())
    if gaps:
        fail(f"the font lacks base code points: {' '.join(u(cp) for cp in gaps)}")
    wanted = (code_points(BASE) | code_points(TIER_A) | code_points(TIER_B)) & source_cmap.keys()
    if set(cmap) != wanted:
        extra = sorted(set(cmap) - wanted)
        lost = sorted(wanted - set(cmap))
        fail(f"the cmap differs from the ranges as Iosevka has them: extra {[u(c) for c in extra[:20]]}, lost {[u(c) for c in lost[:20]]}")

    names = {(r.nameID, r.platformID): r.toUnicode() for r in font["name"].names}
    if sorted({nid for nid, _ in names}) != NAME_IDS:
        fail(f"the name records are {sorted({nid for nid, _ in names})}, not {NAME_IDS}")
    for (nid, platform), text in names.items():
        if nid in RENAMED and text != RENAMED[nid]:
            fail(f"name record {nid} (platform {platform}) says {text!r}, not {RENAMED[nid]!r}")
        if "iosevka" in text.lower():
            fail(f"name record {nid} (platform {platform}) still names Iosevka: {text!r}")

    if len(woff2) > MAX_BYTES:
        fail(f"the woff2 is {len(woff2):,} bytes, more than {MAX_BYTES:,}")
    return cmap


def unicode_range(cmap):
    """The CSS unicode-range value covering exactly the code points of `cmap`."""
    runs = []
    for cp in sorted(cmap):
        if runs and runs[-1][1] == cp - 1:
            runs[-1][1] = cp
        else:
            runs.append([cp, cp])
    return ", ".join(u(lo) if lo == hi else f"{u(lo)}-{hi:04X}" for lo, hi in runs)


def main():
    args = sys.argv[1:]
    if args not in ([], ["--check"]):
        fail("usage: font.py [--check]")
    if fontTools.version != FONTTOOLS_VERSION or brotli.__version__ != BROTLI_VERSION:
        fail(f"run with fontTools {FONTTOOLS_VERSION} and brotli {BROTLI_VERSION}, not {fontTools.version} and {brotli.__version__}")

    archive = fetch(RELEASE + ZIP_NAME, ZIP_SHA256, CACHE / ZIP_NAME)
    licence = fetch(LICENCE_URL, LICENCE_SHA256, CACHE / "LICENSE.md")
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        if MEMBER not in z.namelist():
            fail(f"{ZIP_NAME} has no {MEMBER}")
        ttf = z.read(MEMBER)
    source_cmap = TTFont(io.BytesIO(ttf)).getBestCmap()

    woff2 = build(ttf)
    cmap = check(woff2, source_cmap)
    css = unicode_range(cmap) + "\n"
    glyphs = len(TTFont(io.BytesIO(woff2)).getGlyphOrder())

    if args == ["--check"]:
        with tempfile.NamedTemporaryFile(dir=CACHE, suffix=".woff2") as tmp:
            tmp.write(woff2)
            tmp.flush()
            rebuilt = Path(tmp.name).read_bytes()
        stale = [
            str(path.relative_to(ROOT))
            for path, want in ((WOFF2, rebuilt), (RANGE, css.encode()), (LICENCE, licence))
            if not path.exists() or path.read_bytes() != want
        ]
        if stale:
            fail(f"out of date, rebuild with `mise run build:font`: {', '.join(stale)}")
        print(f"font.py: {WOFF2.relative_to(ROOT)} matches a rebuild ({len(woff2):,} bytes)")
        return

    OUT.mkdir(parents=True, exist_ok=True)
    WOFF2.write_bytes(woff2)
    RANGE.write_text(css)
    LICENCE.write_bytes(licence)
    absent = " ".join(u(cp) for cp in sorted(TIER_A_ABSENT))
    print(f"wrote {WOFF2.relative_to(ROOT)}: {len(woff2):,} bytes, {glyphs:,} glyphs, {len(cmap):,} code points, every glyph {UNITS} units")
    print(f"tier A code points Iosevka lacks: {absent}")
    print(f"unicode-range: {css.strip()};")


if __name__ == "__main__":
    main()
