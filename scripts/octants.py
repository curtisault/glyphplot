#!/usr/bin/env python3
"""Generate the octant table, crates/glyphplot/src/canvas/octants.rs (REQ-CANVAS-030).

An octant cell is 2 x 4 dots, numbered as Unicode numbers the BLOCK OCTANT
characters: 1 2 / 3 4 / 5 6 / 7 8, top to bottom. A dot mask sets bit d - 1 for
dot d. The table gives one code point for each of the 256 masks, chosen by
character name from Unicode 16.0.0's UnicodeData.txt:

- BLOCK OCTANT-<digits> (U+1CD00 to U+1CDE5) covers 230 masks;
- the 26 masks the octant block leaves out are drawn by older characters
  (space, the half, quarter and three-quarter blocks, the quadrants, the full
  block), found below by name.

The script fails unless all 256 masks are covered exactly once.

    python3 scripts/octants.py           write the table
    python3 scripts/octants.py --check   exit 1 when the checked-in table differs

The standard library only. UnicodeData.txt is downloaded once into
target/unicode/ and checked against UCD_SHA256 before it is read.
"""

import hashlib
import pathlib
import re
import sys
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
UCD_URL = "https://www.unicode.org/Public/16.0.0/ucd/UnicodeData.txt"
UCD_PATH = ROOT / "target" / "unicode" / "UnicodeData-16.0.0.txt"
# Pinned from the first download (2026-10-18): `curl -sSfO` of UCD_URL, then
# `shasum -a 256 UnicodeData.txt`.
UCD_SHA256 = "ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f"
OUT_PATH = ROOT / "crates" / "glyphplot" / "src" / "canvas" / "octants.rs"

# The quadrants of a cell as octant dots: each quadrant is 2 x 2 dots.
QUADRANT_DOTS = {
    "UPPER LEFT": "13",
    "UPPER RIGHT": "24",
    "LOWER LEFT": "57",
    "LOWER RIGHT": "68",
}
QUADRANT_NAME = re.compile(r"QUADRANT ((?:UPPER|LOWER) (?:LEFT|RIGHT))((?: AND (?:UPPER|LOWER) (?:LEFT|RIGHT))*)")

# The other characters that draw an octant mask exactly, by name, with their
# dots. Each is a whole number of octant dots: a half or a quarter of the
# cell's width, a quarter, half or three quarters of its height.
NAMED_DOTS = {
    "SPACE": "",
    "FULL BLOCK": "12345678",
    "UPPER HALF BLOCK": "1234",
    "LOWER HALF BLOCK": "5678",
    "LEFT HALF BLOCK": "1357",
    "RIGHT HALF BLOCK": "2468",
    "UPPER ONE QUARTER BLOCK": "12",
    "LOWER ONE QUARTER BLOCK": "78",
    "UPPER THREE QUARTERS BLOCK": "123456",
    "LOWER THREE QUARTERS BLOCK": "345678",
    "LEFT HALF UPPER ONE QUARTER BLOCK": "1",
    "RIGHT HALF UPPER ONE QUARTER BLOCK": "2",
    "LEFT HALF LOWER ONE QUARTER BLOCK": "7",
    "RIGHT HALF LOWER ONE QUARTER BLOCK": "8",
    "MIDDLE LEFT ONE QUARTER BLOCK": "35",
    "MIDDLE RIGHT ONE QUARTER BLOCK": "46",
}


def fail(message):
    print(f"octants: {message}", file=sys.stderr)
    sys.exit(1)


def unicode_data():
    """UnicodeData.txt's text, downloaded once and checked by SHA-256."""
    if not UCD_PATH.exists():
        UCD_PATH.parent.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(UCD_URL, timeout=60) as response:
            body = response.read()
        partial = UCD_PATH.with_suffix(".part")
        partial.write_bytes(body)
        partial.replace(UCD_PATH)
    body = UCD_PATH.read_bytes()
    digest = hashlib.sha256(body).hexdigest()
    if digest != UCD_SHA256:
        fail(f"{UCD_PATH} has SHA-256 {digest}, not the pinned {UCD_SHA256}; delete it to download again")
    return body.decode("utf-8")


def mask_of(dots):
    """The mask of a string of dot digits, each 1 to 8 and each once."""
    if len(set(dots)) != len(dots) or any(d not in "12345678" for d in dots):
        fail(f"bad dot digits {dots!r}")
    return sum(1 << (int(d) - 1) for d in dots)


def table(text):
    """Each mask's (code point, name), or a failure unless all 256 are covered once."""
    names = {}
    found = {}

    def cover(mask, code, name):
        if mask in found:
            fail(f"mask {mask:#04x} is drawn by both {found[mask][1]} and {name}")
        found[mask] = (code, name)

    for line in text.splitlines():
        fields = line.split(";")
        code, name = int(fields[0], 16), fields[1]
        names[name] = code
        octant = re.fullmatch(r"BLOCK OCTANT-(\d+)", name)
        if octant:
            cover(mask_of(octant.group(1)), code, name)
            continue
        quadrant = QUADRANT_NAME.fullmatch(name)
        if quadrant:
            parts = [quadrant.group(1)] + [p for p in quadrant.group(2).split(" AND ") if p]
            cover(mask_of("".join(sorted("".join(QUADRANT_DOTS[p] for p in parts)))), code, name)
    for name, dots in NAMED_DOTS.items():
        if name not in names:
            fail(f"no character is named {name}")
        cover(mask_of(dots), names[name], name)
    missing = [m for m in range(256) if m not in found]
    if missing:
        fail(f"{len(missing)} masks have no character: {', '.join(f'{m:#04x}' for m in missing)}")
    return [found[m] for m in range(256)]


def render(rows):
    """The Rust source of the table."""
    lines = [
        "//! The octant table: one code point for each of the 256 masks of a 2 x 4",
        "//! cell (REQ-CANVAS-030). Generated by `scripts/octants.py` from Unicode",
        "//! 16.0.0's `UnicodeData.txt` by character name; do not edit it by hand.",
        "",
        "/// The code point drawing each octant mask. Bit `d - 1` is dot `d`, numbered",
        "/// 1 2 / 3 4 / 5 6 / 7 8 from the top, as the BLOCK OCTANT names number them.",
        "#[rustfmt::skip]",
        "pub(crate) const OCTANTS: [u32; 256] = [",
    ]
    for mask, (code, name) in enumerate(rows):
        lines.append(f"    0x{code:05X}, // {mask:#04x} {name}")
    lines.append("];")
    return "\n".join(lines) + "\n"


def main(argv):
    check = argv[1:] == ["--check"]
    if argv[1:] and not check:
        fail("usage: python3 scripts/octants.py [--check]")
    source = render(table(unicode_data()))
    if check:
        current = OUT_PATH.read_text(encoding="utf-8") if OUT_PATH.exists() else ""
        if current != source:
            fail(f"{OUT_PATH.relative_to(ROOT)} differs from a regeneration; run python3 scripts/octants.py")
        return
    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUT_PATH.write_text(source, encoding="utf-8")
    print(f"octants: wrote {OUT_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main(sys.argv)
