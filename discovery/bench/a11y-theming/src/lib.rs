//! A wasm module that formats a number, to weigh number formatting by locale.
//! Without `icu`: fixed digits with ASCII grouping by hand (the floor).
//! With `icu`: ICU4X's DecimalFormatter for a locale named at run time (compiled data, every locale).

#[cfg(not(any(feature = "icu", feature = "icu_dt")))]
#[unsafe(no_mangle)]
pub extern "C" fn format(v: i64, out: *mut u8, cap: usize) -> usize {
    let s = v.unsigned_abs().to_string();
    let mut g = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            g.push(',');
        }
        g.push(ch);
    }
    if v < 0 {
        g.insert(0, '\u{2212}');
    }
    write(&g, out, cap)
}

#[cfg(feature = "icu")]
#[unsafe(no_mangle)]
pub extern "C" fn format(v: i64, loc: *const u8, len: usize, out: *mut u8, cap: usize) -> usize {
    use icu_decimal::{input::Decimal, DecimalFormatter};
    let tag = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(loc, len)) };
    let Ok(locale) = tag.parse::<icu_locale_core::Locale>() else { return 0 };
    let Ok(f) = DecimalFormatter::try_new(locale.into(), Default::default()) else { return 0 };
    let d = Decimal::from(v);
    let s = f.format(&d).to_string();
    write(&s, out, cap)
}

#[cfg(feature = "icu_dt")]
#[unsafe(no_mangle)]
pub extern "C" fn format_date(y: i32, m: u8, d: u8, loc: *const u8, len: usize, out: *mut u8, cap: usize) -> usize {
    use icu_calendar::Date;
    use icu_datetime::{fieldsets::YMD, DateTimeFormatter};
    let tag = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(loc, len)) };
    let Ok(locale) = tag.parse::<icu_locale_core::Locale>() else { return 0 };
    let Ok(f) = DateTimeFormatter::try_new(locale.into(), YMD::medium()) else { return 0 };
    let Ok(date) = Date::try_new_iso(y, m, d) else { return 0 };
    let s = f.format(&date).to_string();
    write(&s, out, cap)
}

fn write(s: &str, out: *mut u8, cap: usize) -> usize {
    let n = s.len().min(cap);
    unsafe { core::ptr::copy_nonoverlapping(s.as_ptr(), out, n) };
    n
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "icu_dt")]
    #[test]
    fn dates_by_locale() {
        for tag in ["en-US", "de-DE", "ja-JP", "ar-EG", "fa-IR", "he-IL", "th-TH"] {
            let mut buf = [0u8; 128];
            let n = super::format_date(2026, 10, 18, tag.as_ptr(), tag.len(), buf.as_mut_ptr(), buf.len());
            println!("{tag}\t{}", std::str::from_utf8(&buf[..n]).unwrap());
        }
    }

    #[cfg(feature = "icu")]
    #[test]
    fn formats_by_locale() {
        for tag in ["en-US", "de-DE", "fr-FR", "ar-EG", "hi-IN-u-nu-deva", "en-IN", "fa-IR"] {
            let mut buf = [0u8; 128];
            let n = super::format(-1234567, tag.as_ptr(), tag.len(), buf.as_mut_ptr(), buf.len());
            let s = std::str::from_utf8(&buf[..n]).unwrap();
            println!("{tag}\t{s}");
        }
    }
}
