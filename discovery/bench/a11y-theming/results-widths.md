| label | text | code points | graphemes | Σ char width | str width | zero-width chars |
|---|---|---|---|---|---|---|
| number en-US {} | `-1,234,567.891` | 14 | 14 | 14 | 14 | - |
| number de-DE {} | `-1.234.567,891` | 14 | 14 | 14 | 14 | - |
| number de-CH {} | `-1'234'567.891` | 14 | 14 | 14 | 14 | - |
| number fr-FR {} | `-1<U+202F>234<U+202F>567,891` | 14 | 14 | 14 | 14 | - |
| number en-IN {} | `-12,34,567.891` | 14 | 14 | 14 | 14 | - |
| number ar-EG {} | `<U+061C>-١٬٢٣٤٬٥٦٧٫٨٩١` | 15 | 15 | 14 | 14 | U+061C |
| number ar {} | `<U+200E>-1,234,567.891` | 15 | 15 | 14 | 14 | U+200E |
| number fa-IR {} | `<U+200E>−۱٬۲۳۴٬۵۶۷٫۸۹۱` | 15 | 15 | 14 | 14 | U+200E |
| number he-IL {} | `<U+200E>-1,234,567.891` | 15 | 15 | 14 | 14 | U+200E |
| number hi-IN-u-nu-deva {} | `-१२,३४,५६७.८९१` | 14 | 14 | 14 | 14 | - |
| number th-TH-u-nu-thai {} | `-๑,๒๓๔,๕๖๗.๘๙๑` | 14 | 14 | 14 | 14 | - |
| number ja-JP {} | `-1,234,567.891` | 14 | 14 | 14 | 14 | - |
| number zh-CN {} | `-1,234,567.891` | 14 | 14 | 14 | 14 | - |
| number ar-EG {"style":"percent"} | `<U+061C>-١٢٣٬٤٥٦٬٧٨٩٪<U+061C>` | 15 | 15 | 13 | 13 | U+061C U+061C |
| number en-US {"notation":"compact"} | `-1.2M` | 5 | 5 | 5 | 5 | - |
| number ja-JP {"notation":"compact"} | `-123万` | 5 | 5 | 6 | 6 | - |
| number de-DE {"style":"currency","currency":"EUR"} | `-1.234.567,89<U+00A0>€` | 15 | 15 | 15 | 15 | - |
| number ar-EG {"style":"currency","currency":"EGP"} | `<U+061C>-<U+200F>١٬٢٣٤٬٥٦٧٫٨٩<U+00A0>ج.م.<U+200F>` | 21 | 21 | 18 | 18 | U+061C U+200F U+200F |
| date en-US | `Oct 18, 2026, 2:05 PM` | 21 | 21 | 21 | 21 | - |
| month en-US | `Oct` | 3 | 3 | 3 | 3 | - |
| date en-GB | `18 Oct 2026, 14:05` | 18 | 18 | 18 | 18 | - |
| month en-GB | `Oct` | 3 | 3 | 3 | 3 | - |
| date de-DE | `18.10.2026, 14:05` | 17 | 17 | 17 | 17 | - |
| month de-DE | `Okt` | 3 | 3 | 3 | 3 | - |
| date ja-JP | `2026/10/18 14:05` | 16 | 16 | 16 | 16 | - |
| month ja-JP | `10月` | 3 | 3 | 4 | 4 | - |
| date zh-CN | `2026年10月18日 14:05` | 17 | 17 | 20 | 20 | - |
| month zh-CN | `10月` | 3 | 3 | 4 | 4 | - |
| date ar-EG | `١٨<U+200F>/١٠<U+200F>/٢٠٢٦، ٢:٠٥ م` | 20 | 20 | 18 | 18 | U+200F U+200F |
| month ar-EG | `أكتوبر` | 6 | 6 | 6 | 6 | - |
| date he-IL | `18 באוק׳ 2026, 14:05` | 20 | 20 | 20 | 20 | - |
| month he-IL | `אוק׳` | 4 | 4 | 4 | 4 | - |
| date fa-IR | `۲۶ مهر ۱۴۰۵، ۱۴:۰۵` | 18 | 18 | 18 | 18 | - |
| month fa-IR | `مهر` | 3 | 3 | 3 | 3 | - |
| date ko-KR | `2026. 10. 18. 오후 2:05` | 21 | 21 | 23 | 23 | - |
| month ko-KR | `10월` | 3 | 3 | 4 | 4 | - |
| label precomposed | `Café` | 4 | 4 | 4 | 4 | - |
| label decomposed | `Cafe<U+0301>` | 5 | 4 | 4 | 4 | U+0301 |
| label CJK | `東京証券取引所` | 7 | 7 | 14 | 14 | - |
| label Hangul | `서울` | 2 | 2 | 4 | 4 | - |
| label fullwidth digits | `１２３` | 3 | 3 | 6 | 6 | - |
| label emoji | `👍` | 1 | 1 | 2 | 2 | - |
| label emoji ZWJ family | `👨<U+200D>👩<U+200D>👧` | 5 | 1 | 6 | 2 | U+200D U+200D |
| label flag | `🇯🇵` | 2 | 1 | 2 | 2 | - |
| label emoji text-style | `☀` | 1 | 1 | 1 | 1 | - |
| label emoji VS16 | `☀<U+FE0F>` | 2 | 1 | 1 | 2 | U+FE0F |
| label Hebrew | `מחיר` | 4 | 4 | 4 | 4 | - |
| label Arabic | `السعر` | 5 | 5 | 5 | 5 | - |
| label Devanagari | `हिन<U+094D>दी` | 6 | 2 | 5 | 5 | U+094D |
| label Thai | `ราคา` | 4 | 4 | 4 | 4 | - |
| label Arabic-Indic digits | `٠١٢٣٤٥٦٧٨٩` | 10 | 10 | 10 | 10 | - |
| label Extended Arabic-Indic digits | `۰۱۲۳۴۵۶۷۸۹` | 10 | 10 | 10 | 10 | - |
