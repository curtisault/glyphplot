//! RFC 3339 date-times (`2026-10-18T14:52:00.250Z`, `…+09:00`) as UTC
//! instants: whole seconds since 1970-01-01T00:00:00Z and nanoseconds. The
//! offset written is applied; no zone or clock of the host is read
//! (REQ-DATA-090).

/// Days from 1970-01-01 to a proleptic Gregorian date (the days-from-civil
/// algorithm of Howard Hinnant's "chrono-Compatible Low-Level Date
/// Algorithms").
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `s` as (seconds since the epoch, nanoseconds), or `None` when it is not
/// an RFC 3339 `date-time`. Digits past nanoseconds are dropped; a leap
/// second (`:60`) reads as the next second.
pub(crate) fn parse(s: &str) -> Option<(i64, u32)> {
    let b = s.as_bytes();
    let num = |i: usize, n: usize| -> Option<i64> {
        let mut v = 0i64;
        for k in i..i + n {
            let d = b.get(k)?.wrapping_sub(b'0');
            if d > 9 {
                return None;
            }
            v = v * 10 + d as i64;
        }
        Some(v)
    };
    let at = |i: usize, c: u8| b.get(i) == Some(&c);
    if !(at(4, b'-') && at(7, b'-') && (at(10, b'T') || at(10, b't')) && at(13, b':') && at(16, b':')) {
        return None;
    }
    let (y, mo, d, h, mi, se) = (num(0, 4)?, num(5, 2)?, num(8, 2)?, num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut i = 19;
    let mut nanos = 0u32;
    if at(i, b'.') {
        i += 1;
        let start = i;
        while let Some(c) = b.get(i).filter(|c| c.is_ascii_digit()) {
            if i - start < 9 {
                nanos = nanos * 10 + (c - b'0') as u32;
            }
            i += 1;
        }
        if i == start {
            return None;
        }
        for _ in (i - start)..9 {
            nanos *= 10;
        }
    }
    let offset = match *b.get(i)? {
        b'Z' | b'z' if i + 1 == b.len() => 0,
        sign @ (b'+' | b'-') if i + 6 == b.len() && at(i + 3, b':') => {
            let (oh, om) = (num(i + 1, 2)?, num(i + 4, 2)?);
            if oh > 23 || om > 59 {
                return None;
            }
            let o = oh * 3600 + om * 60;
            if sign == b'-' {
                -o
            } else {
                o
            }
        }
        _ => return None,
    };
    Some((days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se - offset, nanos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rfc_3339() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 10, 18), 20_744);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(parse("2026-10-18T00:00:00Z"), Some((1_792_281_600, 0)));
        assert_eq!(parse("2026-10-18t09:00:00.25+09:00"), Some((1_792_281_600, 250_000_000)));
        assert_eq!(parse("2026-10-18T00:00:00.0000000041234z"), Some((1_792_281_600, 4)));
        assert_eq!(parse("2024-02-29T23:59:60-00:30"), Some((days_from_civil(2024, 3, 1) * 86_400 + 1800, 0)));
        for bad in [
            "yesterday",
            "2026-10-18",
            "2026-10-18T00:00:00",
            "2026-13-01T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "2026-10-18T00:00:00.Z",
            "2026-10-18T00:00:00Zx",
            "2026-10-18 00:00:00Z",
            "2026-10-18T24:00:00Z",
            "2026-10-18T00:00:00+0900",
            "",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }
}
