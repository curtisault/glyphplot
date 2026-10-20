// The bundled font's @font-face (REQ-FONT-030, REQ-EL-010): family
// "glyphplot mono", from web/fonts/glyphplot-mono.woff2 with font-display
// swap, limited to the code points the font holds. UNICODE_RANGE is
// web/fonts/unicode-range.txt as written by scripts/font.py;
// web/test/font.test.mjs fails when the two differ.

/** The font's family name, first in every surface's font stack. */
export const FONT_FAMILY = "glyphplot mono";

/** web/fonts/unicode-range.txt. */
export const UNICODE_RANGE =
  "U+0020-007E, U+00A0-017F, U+0394, U+03A3, U+03A9, U+03BC, U+03C3, U+2007-200A, U+2010-2015, U+2018-201F, U+2022, U+2026, U+202F-2030, U+2032-2033, U+2070-2071, U+2074-209F, U+20A1, U+20A3-20AF, U+20B1-20B5, U+20B8-20BA, U+20BC-20C0, U+2116, U+2122, U+2190-21FF, U+2212, U+221E, U+2248, U+2260, U+2264-2265, U+2500-25FF, U+2605-2606, U+2713, U+2717, U+2800-28FF, U+FFFD, U+1CD00-1CDE5, U+1CEA0-1CEAF, U+1FB00-1FB92, U+1FB94-1FBCA, U+1FBCC, U+1FBCE-1FBF9";

/** The font file's URL, beside this module's package. */
export function fontUrl(): string {
  return new URL("../fonts/glyphplot-mono.woff2", import.meta.url).href;
}

/** The @font-face rule. */
export function fontFace(url: string = fontUrl()): string {
  return `@font-face{font-family:"${FONT_FAMILY}";src:url("${url}") format("woff2");font-display:swap;font-style:normal;font-weight:400;unicode-range:${UNICODE_RANGE}}`;
}
