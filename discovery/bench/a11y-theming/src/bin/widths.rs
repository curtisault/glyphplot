//! Reads lines (`label<TAB>text`) on stdin; prints, for each text, its code points,
//! grapheme clusters, the sum of per-char widths (one cell a code point, as a grid
//! that holds one code point a cell would lay it out) and unicode-width's string width.
use std::io::BufRead;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn main() {
    println!("| label | text | code points | graphemes | Σ char width | str width | zero-width chars |");
    println!("|---|---|---|---|---|---|---|");
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let Some((label, text)) = line.split_once('\t') else { continue };
        let cps = text.chars().count();
        let gr = text.graphemes(true).count();
        let sum: usize = text.chars().map(|c| c.width().unwrap_or(0)).sum();
        let zw: Vec<String> = text.chars().filter(|c| c.width().unwrap_or(0) == 0).map(|c| format!("U+{:04X}", c as u32)).collect();
        let shown: String = text.chars().map(|c| if c.width().unwrap_or(0) == 0 || c.is_whitespace() && c != ' ' { format!("<U+{:04X}>", c as u32) } else { c.to_string() }).collect();
        println!("| {label} | `{shown}` | {cps} | {gr} | {sum} | {} | {} |", text.width(), if zw.is_empty() { "-".into() } else { zw.join(" ") });
    }
}
