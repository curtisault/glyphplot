//! Step 3: a UTC time scale with calendar ticks (seconds to years) and date
//! formatting, by hand. Times are milliseconds since the Unix epoch.

use crate::scale::push_uint;

const S: f64 = 1000.0;
const M: f64 = 60.0 * S;
const H: f64 = 60.0 * M;
const D: f64 = 24.0 * H;

/// Fixed intervals in ms, then months (negative: -k is k months).
const INTERVALS: [f64; 22] =
    [S, 5.0 * S, 15.0 * S, 30.0 * S, M, 5.0 * M, 15.0 * M, 30.0 * M, H, 3.0 * H, 6.0 * H, 12.0 * H, D, 2.0 * D, 7.0 * D, -1.0, -3.0, -6.0, -12.0, -24.0, -60.0, -120.0];

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// (year, month 1..=12, day) from days since 1970-01-01, after Howard Hinnant's
/// civil_from_days ("chrono-Compatible Low-Level Date Algorithms"): count from
/// 0000-03-01 so each year ends with February, split into 400-year cycles of
/// 146,097 days, then into years and into five-month runs of 153 days.
pub fn civil(days: i64) -> (i64, u32, u32) {
    const CYCLE: i64 = 146_097; // days in 400 Gregorian years
    const MARCH_1_YEAR_0: i64 = 719_468; // days from 0000-03-01 to 1970-01-01
    let since = days + MARCH_1_YEAR_0;
    let cycle = since.div_euclid(CYCLE);
    let day_of_cycle = since.rem_euclid(CYCLE); // 0..=146_096
    // Leap days drop out by subtracting a day each 4 years, adding one back each
    // 100, and subtracting one for the cycle's last day.
    let year_of_cycle = (day_of_cycle - day_of_cycle / 1_460 + day_of_cycle / 36_524 - day_of_cycle / (CYCLE - 1)) / 365;
    let day_of_year = day_of_cycle - (year_of_cycle * 365 + year_of_cycle / 4 - year_of_cycle / 100); // 0 = March 1
    let month_from_march = (day_of_year * 5 + 2) / 153; // 0 = March .. 11 = February
    let day = day_of_year - (month_from_march * 153 + 2) / 5 + 1;
    let month = (month_from_march + 2) % 12 + 1;
    let year = cycle * 400 + year_of_cycle + i64::from(month < 3);
    (year, month as u32, day as u32)
}

/// Days since 1970-01-01 of a civil date (Hinnant's days_from_civil).
pub fn days(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Tick times over `[a, b]` for about `n` ticks, and the interval chosen.
pub fn ticks(a: f64, b: f64, n: usize, out: &mut Vec<f64>) -> f64 {
    out.clear();
    let target = (b - a) / n.max(1) as f64;
    let iv = INTERVALS.iter().copied().find(|&iv| if iv > 0.0 { iv >= target } else { -iv * 30.0 * D >= target }).unwrap_or(-120.0);
    if iv > 0.0 {
        let mut t = (a / iv).ceil() * iv;
        while t <= b && out.len() < 64 {
            out.push(t);
            t += iv;
        }
    } else {
        let k = (-iv) as i64;
        let (y, m, _) = civil((a / D).floor() as i64);
        let mut mi = y * 12 + i64::from(m) - 1;
        mi = mi.div_euclid(k) * k;
        loop {
            let t = days(mi.div_euclid(12), (mi.rem_euclid(12) + 1) as u32, 1) as f64 * D;
            if t > b || out.len() >= 64 {
                break;
            }
            if t >= a {
                out.push(t);
            }
            mi += k;
        }
    }
    iv
}

fn two(s: &mut String, v: u32) {
    if v < 10 {
        s.push('0');
    }
    push_uint(s, u64::from(v));
}

/// A tick label at the precision the interval needs.
pub fn label(s: &mut String, t: f64, iv: f64) {
    let ms = t.floor() as i64;
    let day = ms.div_euclid(86_400_000);
    let (y, m, d) = civil(day);
    let sec = (ms.rem_euclid(86_400_000) / 1000) as u32;
    if iv > 0.0 && iv < D {
        two(s, sec / 3600);
        s.push(':');
        two(s, sec / 60 % 60);
        if iv < M {
            s.push(':');
            two(s, sec % 60);
        }
    } else if iv > 0.0 || iv > -12.0 && m != 1 {
        s.push_str(MONTHS[(m - 1) as usize]);
        if iv > 0.0 {
            s.push(' ');
            push_uint(s, u64::from(d));
        }
    } else {
        push_uint(s, y.max(0) as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(days(2026, 10, 18), 20_744);
        assert_eq!(civil(20_744), (2026, 10, 18));
        let mut out = vec![];
        let iv = ticks(0.0, 400.0 * D, 4, &mut out);
        assert_eq!(iv, -6.0);
        let mut s = String::new();
        label(&mut s, out[1], iv);
        assert_eq!(s, "Jul");
        s.clear();
        label(&mut s, 3_723_000.0, 15.0 * S);
        assert_eq!(s, "01:02:03");
    }
}
