//! Step 2: a linear scale with nice ticks (d3-array's tickIncrement, by hand)
//! and number formatting with SI prefixes, written without core::fmt's float
//! formatting (unless the `fmtfloat` feature asks for it, to measure it).

pub struct Ticks {
    pub start: f64,
    pub step: f64,
    pub count: usize,
}

fn pow10(e: i32) -> f64 {
    let mut p = 1.0;
    if e >= 0 {
        for _ in 0..e {
            p *= 10.0;
        }
    } else {
        for _ in 0..-e {
            p /= 10.0;
        }
    }
    p
}

/// A nice step (1, 2 or 5 times a power of ten) for about `n` ticks over `[a, b]`.
pub fn nice_step(a: f64, b: f64, n: usize) -> f64 {
    let raw = (b - a).abs() / n.max(1) as f64;
    if !(raw > 0.0) || !raw.is_finite() {
        return 1.0;
    }
    let e = raw.log10().floor() as i32;
    let base = pow10(e);
    let f = raw / base;
    let m = if f >= 7.07 {
        10.0
    } else if f >= 3.16 {
        5.0
    } else if f >= 1.41 {
        2.0
    } else {
        1.0
    };
    m * base
}

/// Widens `[a, b]` to multiples of its nice step.
pub fn nice(a: f64, b: f64, n: usize) -> (f64, f64) {
    let s = nice_step(a, b, n);
    ((a / s).floor() * s, (b / s).ceil() * s)
}

pub fn ticks(a: f64, b: f64, n: usize) -> Ticks {
    let step = nice_step(a, b, n);
    let start = (a / step).ceil() * step;
    let count = (((b - start) / step).floor() as i64 + 1).max(0) as usize;
    Ticks { start, step, count: count.min(64) }
}

pub fn push_uint(s: &mut String, mut v: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for &b in &buf[i..] {
        s.push(b as char);
    }
}

/// `v` with `decimals` digits after the point, rounded half away from zero.
pub fn fixed(s: &mut String, v: f64, decimals: u32) {
    #[cfg(feature = "fmtfloat")]
    {
        use std::fmt::Write;
        let _ = write!(s, "{:.*}", decimals as usize, v);
    }
    #[cfg(not(feature = "fmtfloat"))]
    {
        if !v.is_finite() {
            s.push_str(if v.is_nan() { "NaN" } else if v > 0.0 { "∞" } else { "-∞" });
            return;
        }
        let scale = pow10(decimals as i32);
        let m = (v.abs() * scale).round();
        if v < 0.0 && m != 0.0 {
            s.push('-');
        }
        let m = m as u64;
        let p = scale as u64;
        push_uint(s, m / p);
        if decimals > 0 {
            s.push('.');
            let frac = m % p;
            let mut digits = String::new();
            push_uint(&mut digits, frac);
            for _ in digits.len()..decimals as usize {
                s.push('0');
            }
            s.push_str(&digits);
        }
    }
}

const PREFIXES: [&str; 11] = ["p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E"];

/// A tick label: SI-prefixed when the magnitude calls for it, with as many
/// decimals as the tick step needs and no more.
pub fn tick_label(s: &mut String, v: f64, step: f64) {
    let mag = v.abs().max(step.abs());
    let g = if mag > 0.0 && mag.is_finite() { ((mag.log10().floor() as i32).div_euclid(3)).clamp(-4, 6) } else { 0 };
    let g = if g == -1 || g == 0 && mag >= 1.0 { 0 } else { g };
    let div = pow10(g * 3);
    let st = step / div;
    let decimals = if st > 0.0 && st.is_finite() { (-(st.log10().floor() as i32)).max(0) as u32 } else { 0 };
    fixed(s, v / div, decimals.min(9));
    s.push_str(PREFIXES[(g + 4) as usize]);
}

/// A readout value: `digits` significant digits with an SI prefix.
pub fn si(s: &mut String, v: f64, digits: u32) {
    if v == 0.0 || !v.is_finite() {
        fixed(s, v, 0);
        return;
    }
    let e = v.abs().log10().floor() as i32;
    let g = e.div_euclid(3).clamp(-4, 6);
    let scaled = v / pow10(g * 3);
    let int_digits = (e - g * 3 + 1).max(1) as u32;
    fixed(s, scaled, digits.saturating_sub(int_digits));
    s.push_str(PREFIXES[(g + 4) as usize]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(v: f64, step: f64) -> String {
        let mut s = String::new();
        tick_label(&mut s, v, step);
        s
    }

    #[test]
    fn labels() {
        assert_eq!(label(0.5, 0.1), "0.5");
        assert_eq!(label(2000.0, 500.0), "2.0k");
        assert_eq!(label(3_000_000.0, 1_000_000.0), "3M");
        assert_eq!(label(-20.0, 10.0), "-20");
        let mut s = String::new();
        si(&mut s, 0.00123, 3);
        assert_eq!(s, "1.23m");
        let t = ticks(0.0, 97.0, 5);
        assert_eq!((t.start, t.step, t.count), (0.0, 20.0, 5));
    }
}
