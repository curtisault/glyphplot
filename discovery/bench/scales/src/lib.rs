//! Throwaway: one exported function, `run`, whose body depends on the Cargo
//! feature built. Each variant makes one real call into one dependency, so the
//! gzipped module's growth over `base` is what that dependency costs.
//! `x` is a number, `t` milliseconds since the epoch, `s` a locale or zone name.

pub mod hand;

use wasm_bindgen::prelude::*;

#[cfg(feature = "empty")]
#[wasm_bindgen]
pub fn run(x: f64, t: f64) -> f64 {
    x + t
}

#[cfg(feature = "base")]
#[wasm_bindgen]
pub fn run(x: f64, t: f64, s: &str) -> String {
    let mut out = format!("{}|{}|{}|", x as i64, t as i64, s);
    variant(x, t, s, &mut out);
    out
}

#[cfg(all(feature = "base", not(any(
    feature = "fmt-float",
    feature = "ryu",
    feature = "num-format-en",
    feature = "num-format-any",
    feature = "chrono",
    feature = "chrono-tz",
    feature = "time",
    feature = "jiff-utc",
    feature = "jiff-civil",
    feature = "jiff-tz",
    feature = "icu-decimal",
    feature = "icu-datetime",
    feature = "js-intl",
    feature = "hand",
    feature = "hand-linear",
    feature = "hand-time"
))))]
fn variant(_x: f64, _t: f64, _s: &str, _out: &mut String) {}

/// core::fmt's float formatting, as `format!("{:.2}")` pulls it in.
#[cfg(feature = "fmt-float")]
fn variant(x: f64, _t: f64, _s: &str, out: &mut String) {
    out.push_str(&format!("{x:.2} {x}"));
}

#[cfg(feature = "ryu")]
fn variant(x: f64, _t: f64, _s: &str, out: &mut String) {
    out.push_str(ryu::Buffer::new().format(x));
}

/// Digit grouping with one locale fixed at compile time.
#[cfg(feature = "num-format-en")]
fn variant(x: f64, _t: f64, _s: &str, out: &mut String) {
    use num_format::{Locale, ToFormattedString};
    out.push_str(&(x as i64).to_formatted_string(&Locale::en));
}

/// Digit grouping with the locale chosen at run time by name.
#[cfg(feature = "num-format-any")]
fn variant(x: f64, _t: f64, s: &str, out: &mut String) {
    use num_format::{Locale, ToFormattedString};
    let locale = Locale::from_name(s).unwrap_or(Locale::en);
    out.push_str(&(x as i64).to_formatted_string(&locale));
}

#[cfg(feature = "chrono")]
fn variant(_x: f64, t: f64, _s: &str, out: &mut String) {
    if let Some(d) = chrono::DateTime::from_timestamp_millis(t as i64) {
        out.push_str(&d.format("%Y-%m-%d %H:%M").to_string());
    }
}

/// A zone chosen at run time by IANA name: every zone's rules are linked.
#[cfg(feature = "chrono-tz")]
fn variant(_x: f64, t: f64, s: &str, out: &mut String) {
    let tz: chrono_tz::Tz = s.parse().unwrap_or(chrono_tz::UTC);
    if let Some(d) = chrono::DateTime::from_timestamp_millis(t as i64) {
        out.push_str(&d.with_timezone(&tz).format("%Y-%m-%d %H:%M %Z").to_string());
    }
}

#[cfg(feature = "time")]
fn variant(_x: f64, t: f64, _s: &str, out: &mut String) {
    let fmt = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]");
    if let Ok(d) = time::OffsetDateTime::from_unix_timestamp_nanos(t as i128 * 1_000_000) {
        if let Ok(text) = d.format(&fmt) {
            out.push_str(&text);
        }
    }
}

#[cfg(feature = "jiff-utc")]
fn variant(_x: f64, t: f64, _s: &str, out: &mut String) {
    if let Ok(ts) = jiff::Timestamp::from_millisecond(t as i64) {
        out.push_str(&ts.to_zoned(jiff::tz::TimeZone::UTC).strftime("%Y-%m-%d %H:%M").to_string());
    }
}

/// A zone chosen at run time by IANA name, from the bundled tzdb.
#[cfg(feature = "jiff-tz")]
fn variant(_x: f64, t: f64, s: &str, out: &mut String) {
    let tz = jiff::tz::TimeZone::get(s).unwrap_or(jiff::tz::TimeZone::UTC);
    if let Ok(ts) = jiff::Timestamp::from_millisecond(t as i64) {
        out.push_str(&ts.to_zoned(tz).strftime("%Y-%m-%d %H:%M %Z").to_string());
    }
}

/// Locale-aware decimal formatting, the locale chosen at run time.
#[cfg(feature = "icu-decimal")]
fn variant(x: f64, _t: f64, s: &str, out: &mut String) {
    use icu_decimal::DecimalFormatter;
    let locale: icu_locale_core::Locale = s.parse().unwrap_or(icu_locale_core::Locale::UNKNOWN);
    if let Ok(f) = DecimalFormatter::try_new(locale.into(), Default::default()) {
        let mut d = fixed_decimal::Decimal::from((x * 100.0) as i64);
        d.multiply_pow10(-2);
        out.push_str(&f.format(&d).to_string());
    }
}

/// Locale-aware date and time, Gregorian only, the locale chosen at run time.
#[cfg(feature = "icu-datetime")]
fn variant(_x: f64, t: f64, s: &str, out: &mut String) {
    use icu_datetime::fieldsets::YMD;
    use icu_datetime::input::{Date, DateTime, Time};
    use icu_datetime::FixedCalendarDateTimeFormatter;
    let locale: icu_locale_core::Locale = s.parse().unwrap_or(icu_locale_core::Locale::UNKNOWN);
    let days = (t / 86_400_000.0).floor() as i64;
    let (y, m, d) = civil(days);
    let secs = ((t / 1000.0).floor() as i64).rem_euclid(86_400);
    if let (Ok(date), Ok(time)) = (Date::try_new_gregorian(y as i32, m as u8, d as u8), Time::try_new((secs / 3600) as u8, (secs / 60 % 60) as u8, 0, 0)) {
        if let Ok(f) = FixedCalendarDateTimeFormatter::try_new(locale.into(), YMD::medium().with_time_hm()) {
            out.push_str(&f.format(&DateTime { date, time }).to_string());
        }
    }
}

#[cfg(feature = "icu-datetime")]
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The browser's Intl through js-sys: the wasm side of asking JS.
#[cfg(feature = "js-intl")]
fn variant(_x: f64, t: f64, s: &str, out: &mut String) {
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"timeZone".into(), &s.into());
    let _ = js_sys::Reflect::set(&opts, &"dateStyle".into(), &"medium".into());
    let _ = js_sys::Reflect::set(&opts, &"timeStyle".into(), &"short".into());
    let f = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::of1(&"en-US".into()), &opts);
    let date = js_sys::Date::new(&t.into());
    if let Some(text) = f.format().call1(&f, &date).ok().and_then(|v| v.as_string()) {
        out.push_str(&text);
    }
}

/// Hand-written: nice domain, ticks, SI labels, UTC calendar ticks and labels.
#[cfg(feature = "hand")]
fn variant(x: f64, t: f64, _s: &str, out: &mut String) {
    let (a, b) = hand::nice(-x, x * 3.0, 10.0);
    let f = hand::SiFormat::new(a, b, 10.0);
    for v in hand::ticks(a, b, 10.0) {
        f.format(v, out);
        out.push(' ');
    }
    for v in hand::utc_ticks(t, t + x * 1000.0, 10.0) {
        hand::utc_label(v, out);
        out.push(' ');
    }
}

/// Hand-written: nice domain, ticks and SI labels only.
#[cfg(feature = "hand-linear")]
fn variant(x: f64, _t: f64, _s: &str, out: &mut String) {
    let (a, b) = hand::nice(-x, x * 3.0, 10.0);
    let f = hand::SiFormat::new(a, b, 10.0);
    for v in hand::ticks(a, b, 10.0) {
        f.format(v, out);
        out.push(' ');
    }
}

/// Hand-written: UTC calendar ticks and labels only.
#[cfg(feature = "hand-time")]
fn variant(x: f64, t: f64, _s: &str, out: &mut String) {
    for v in hand::utc_ticks(t, t + x * 1000.0, 10.0) {
        hand::utc_label(v, out);
        out.push(' ');
    }
}

/// jiff without Zoned or strftime: a fixed offset to civil fields, then Display.
#[cfg(feature = "jiff-civil")]
fn variant(_x: f64, t: f64, _s: &str, out: &mut String) {
    if let Ok(ts) = jiff::Timestamp::from_millisecond(t as i64) {
        let dt = jiff::tz::Offset::UTC.to_datetime(ts);
        out.push_str(&format!("{} {:02}:{:02}", dt.date(), dt.hour(), dt.minute()));
    }
}
