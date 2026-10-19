//! JavaScript's number semantics where d3 relies on them, written by hand so
//! the core matches d3 bit for bit and never formats a float with `core::fmt`
//! (REQ-FMT-030, REQ-TICK-010).
//!
//! - `Math.round`, `Math.max`, `Math.min`, `Math.pow(10, n)` and
//!   `Math.floor(Math.log10(x))`, as ECMAScript 2024 §21.3.2 defines them and
//!   V8 14.6 (Node 26) computes them: `Math.pow(10, n)` correctly rounded for
//!   every integer n (checked in Node 26.5.1 for n from −340 to 310), and
//!   `Math.log10` as fdlibm's (`fdlibm.rs`).
//! - `Number.prototype.toFixed`, `toExponential`, `toPrecision` and
//!   `toString` (ECMAScript §21.1.3), from the exact decimal expansion of the
//!   `f64`: fixed digits round half up, as the specification says, and the
//!   shortest digits are Burger and Dybvig's free-format algorithm ("Printing
//!   Floating-Point Numbers Quickly and Accurately", PLDI 1996) with V8's
//!   choice on a tie (the even digit).

use super::big::{compose, Big};

/// JavaScript's `Math.round`: the nearest integer, a half going up (towards
/// +∞), and -0 kept for -0.5 ≤ x < 0, where `f64::round` sends halves away
/// from zero.
pub fn round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    if r == 0.0 && x.is_sign_negative() {
        -0.0
    } else {
        r
    }
}

/// JavaScript's `Math.max` of two numbers: NaN wins, and +0 beats -0.
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b || (a == b && b.is_sign_negative()) {
        a
    } else {
        b
    }
}

/// JavaScript's `Math.min` of two numbers: NaN wins, and -0 beats +0.
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

/// 10ⁿ for |n| ≤ 24, each literal correctly rounded by the compiler.
const POS: [f64; 25] =
    [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22, 1e23, 1e24];
const NEG: [f64; 25] = [
    1e0, 1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-11, 1e-12, 1e-13, 1e-14, 1e-15, 1e-16, 1e-17, 1e-18, 1e-19, 1e-20, 1e-21, 1e-22,
    1e-23, 1e-24,
];

/// JavaScript's `Math.pow(10, n)` for an integer, infinite or NaN `n`:
/// 10ⁿ correctly rounded, as V8 gives it for every integer `n`.
pub fn pow10(n: f64) -> f64 {
    if n.is_nan() {
        return f64::NAN;
    }
    if n > 308.0 {
        return f64::INFINITY;
    }
    if n < -343.0 {
        return 0.0;
    }
    let i = n as i32;
    if (0..=24).contains(&i) {
        POS[i as usize]
    } else if (-24..0).contains(&i) {
        NEG[(-i) as usize]
    } else if i > 0 {
        Big::pow10(i as u32).to_f64_scaled(0)
    } else {
        // 1 / 10^|i|: the top 64 bits of 2^b / 10^|i| by long division, with
        // b the bit length of 10^|i|, and whether a remainder is left.
        let d = Big::pow10((-i) as u32);
        let b = d.bit_len();
        let mut r = Big::from_u64(1);
        r.shl(b);
        let mut q = 0u64;
        for _ in 0..64 {
            q <<= 1;
            if r >= d {
                r.sub(&d);
                q |= 1;
            }
            r.shl(1);
        }
        compose(q, -63 - b as i32, !r.is_zero())
    }
}

/// JavaScript's `Math.log10(x)` as V8 computes it (fdlibm's), not the
/// platform's.
pub fn log10(x: f64) -> f64 {
    super::fdlibm::log10(x)
}

/// JavaScript's `Math.floor(Math.log10(x))`.
pub fn floor_log10(x: f64) -> f64 {
    log10(x).floor()
}

/// `x = f × 2^e` exactly, for finite x > 0.
fn decompose(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1 << 52) - 1);
    if exp == 0 {
        (frac, -1074)
    } else {
        (frac | (1 << 52), exp - 1075)
    }
}

/// `x / 10^k` as `r / s` with `s/10 ≤ r < s`, for finite x > 0.
fn scaled(x: f64) -> (Big, Big, i32) {
    let (f, e) = decompose(x);
    let (mut r, mut s) = (Big::from_u64(f), Big::from_u64(1));
    if e >= 0 {
        r.shl(e as u32);
    } else {
        s.shl((-e) as u32);
    }
    let mut k = estimate_k(f, e);
    if k >= 0 {
        s.mul_pow10(k as u32);
    } else {
        r.mul_pow10((-k) as u32);
    }
    while r >= s {
        s.mul_small(10);
        k += 1;
    }
    loop {
        let mut r10 = r.clone();
        r10.mul_small(10);
        if r10 >= s {
            break;
        }
        r = r10;
        k -= 1;
    }
    (r, s, k)
}

/// About ⌈log10(f × 2^e)⌉, from the bit length; the callers correct it.
fn estimate_k(f: u64, e: i32) -> i32 {
    let bits = 64 - f.leading_zeros() as i32;
    (f64::from(e + bits) * core::f64::consts::LOG10_2).ceil() as i32
}

/// Adds one unit in the last digit, carrying. Returns true when the carry
/// ran off the front: the digits are then `1` followed by zeros, one longer.
fn increment(digits: &mut Vec<u8>) -> bool {
    for d in digits.iter_mut().rev() {
        if *d == 9 {
            *d = 0;
        } else {
            *d += 1;
            return false;
        }
    }
    digits.insert(0, 1);
    true
}

/// The first `n` digits of `r / s` (in [0.1, 1)), the n-th rounded half up
/// on the exact remainder. Returns the digits and whether a carry added one
/// in front (the value's decimal exponent is then one higher).
fn fixed_digits(mut r: Big, s: &Big, n: usize) -> (Vec<u8>, bool) {
    let mut digits = Vec::with_capacity(n + 1);
    for _ in 0..n {
        r.mul_small(10);
        digits.push(r.div_digit(s));
    }
    r.mul_small(2);
    let carry = r >= *s && increment(&mut digits);
    (digits, carry)
}

/// The shortest digits that read back as `x` (finite, > 0), and their
/// decimal exponent k: x ≈ 0.d₁d₂… × 10^k.
fn shortest_digits(x: f64) -> (Vec<u8>, i32) {
    let (f, e) = decompose(x);
    let even = f & 1 == 0;
    let one = || Big::from_u64(1);
    // r / s = x; mp and mm are the gaps to the neighbours, both over s, ×2.
    let (mut r, mut s, mut mp, mut mm);
    if e >= 0 {
        let mut be = one();
        be.shl(e as u32);
        if f != 1 << 52 {
            r = Big::from_u64(f);
            r.shl(e as u32 + 1);
            s = Big::from_u64(2);
            mp = be.clone();
            mm = be;
        } else {
            r = Big::from_u64(f);
            r.shl(e as u32 + 2);
            s = Big::from_u64(4);
            mp = be.clone();
            mp.shl(1);
            mm = be;
        }
    } else if e == -1074 || f != 1 << 52 {
        r = Big::from_u64(f);
        r.shl(1);
        s = one();
        s.shl((1 - e) as u32);
        mp = one();
        mm = one();
    } else {
        r = Big::from_u64(f);
        r.shl(2);
        s = one();
        s.shl((2 - e) as u32);
        mp = Big::from_u64(2);
        mm = one();
    }
    let mut k = estimate_k(f, e);
    if k >= 0 {
        s.mul_pow10(k as u32);
    } else {
        let p = (-k) as u32;
        r.mul_pow10(p);
        mp.mul_pow10(p);
        mm.mul_pow10(p);
    }
    let high_reaches = |r: &Big, mp: &Big, s: &Big| {
        let mut h = r.clone();
        h.add(mp);
        if even {
            h >= *s
        } else {
            h > *s
        }
    };
    while high_reaches(&r, &mp, &s) {
        s.mul_small(10);
        k += 1;
    }
    loop {
        let (mut r10, mut mp10) = (r.clone(), mp.clone());
        r10.mul_small(10);
        mp10.mul_small(10);
        if high_reaches(&r10, &mp10, &s) {
            break;
        }
        r = r10;
        mp = mp10;
        mm.mul_small(10);
        k -= 1;
    }
    let mut digits = Vec::with_capacity(17);
    loop {
        r.mul_small(10);
        mp.mul_small(10);
        mm.mul_small(10);
        let d = r.div_digit(&s);
        let low = if even { r <= mm } else { r < mm };
        let high = high_reaches(&r, &mp, &s);
        digits.push(d);
        if !low && !high {
            continue;
        }
        let up = if low && high {
            let mut r2 = r.clone();
            r2.mul_small(2);
            match r2.cmp(&s) {
                core::cmp::Ordering::Less => false,
                core::cmp::Ordering::Greater => true,
                core::cmp::Ordering::Equal => d % 2 == 1,
            }
        } else {
            high
        };
        if up && increment(&mut digits) {
            digits.pop();
            k += 1;
        }
        break;
    }
    (digits, k)
}

fn push_digits(digits: &[u8], out: &mut String) {
    out.extend(digits.iter().map(|&d| char::from(b'0' + d)));
}

fn push_zeros(n: usize, out: &mut String) {
    out.extend(core::iter::repeat_n('0', n));
}

fn push_int(mut n: u32, out: &mut String) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out.extend(buf[i..].iter().map(|&b| char::from(b)));
}

/// `e+3`, `e-7`, `e+0`.
fn push_exponent(e: i32, out: &mut String) {
    out.push('e');
    out.push(if e < 0 { '-' } else { '+' });
    push_int(e.unsigned_abs(), out);
}

/// The digits of `x` (finite, > 0) to `p` significant places if given,
/// else the shortest that read back, with the decimal exponent of the first:
/// d3-format's `formatDecimalParts(x, p)`, from `x.toExponential(p − 1)`.
pub fn decimal_parts(x: f64, p: Option<usize>) -> (Vec<u8>, i32) {
    match p {
        Some(p) if p > 0 => {
            let (r, s, k) = scaled(x);
            let (mut digits, carry) = fixed_digits(r, &s, p);
            if carry {
                digits.truncate(p);
            }
            (digits, k - 1 + i32::from(carry))
        }
        _ => {
            let (digits, k) = shortest_digits(x);
            (digits, k - 1)
        }
    }
}

/// d3-format's `exponent(x)`: the decimal exponent of |x| written shortest,
/// or None for 0 and non-finite x (where d3 gives NaN).
pub fn exponent(x: f64) -> Option<i32> {
    let x = x.abs();
    if x == 0.0 || !x.is_finite() {
        None
    } else {
        Some(decimal_parts(x, None).1)
    }
}

/// `Number.prototype.toFixed(f)` for finite x ≥ 0 and f ≤ 100: the integer
/// n nearest x·10^f (the larger on a tie) with the point put back; from
/// 10²¹ up, `toString`.
pub fn to_fixed(x: f64, f: usize, out: &mut String) {
    if x >= 1e21 {
        return to_string(x, out);
    }
    let digits = if x == 0.0 {
        Vec::new()
    } else {
        let (r, s, k) = scaled(x);
        let n = k + f as i32;
        if n < 0 {
            Vec::new()
        } else {
            fixed_digits(r, &s, n as usize).0
        }
    };
    // The integer n's digits, with zeros in front so one is left of the point.
    let pad = (f + 1).saturating_sub(digits.len());
    let int_len = pad + digits.len() - f;
    let all = core::iter::repeat_n(0u8, pad).chain(digits.iter().copied());
    for (i, d) in all.enumerate() {
        if i == int_len {
            out.push('.');
        }
        out.push(char::from(b'0' + d));
    }
}

/// `Number.prototype.toExponential(f)` for finite x ≥ 0: f digits after the
/// point (the larger on a tie), or as many as `toString` needs when None.
pub fn to_exponential(x: f64, f: Option<usize>, out: &mut String) {
    let (digits, e) = if x == 0.0 { (vec![0; f.unwrap_or(0) + 1], 0) } else { decimal_parts(x, f.map(|f| f + 1)) };
    out.push(char::from(b'0' + digits[0]));
    if digits.len() > 1 {
        out.push('.');
        push_digits(&digits[1..], out);
    }
    push_exponent(e, out);
}

/// `Number.prototype.toPrecision(p)` for finite x ≥ 0 and 1 ≤ p ≤ 100.
pub fn to_precision(x: f64, p: usize, out: &mut String) {
    let p = p.max(1);
    let (digits, e) = if x == 0.0 { (vec![0; p], 0) } else { decimal_parts(x, Some(p)) };
    if e < -6 || e >= p as i32 {
        out.push(char::from(b'0' + digits[0]));
        if p > 1 {
            out.push('.');
            push_digits(&digits[1..], out);
        }
        push_exponent(e, out);
    } else if e >= 0 {
        let int = e as usize + 1;
        push_digits(&digits[..int], out);
        if int < p {
            out.push('.');
            push_digits(&digits[int..], out);
        }
    } else {
        out.push_str("0.");
        push_zeros((-(e + 1)) as usize, out);
        push_digits(&digits, out);
    }
}

/// `Number.prototype.toString()` (radix 10) for finite x: the shortest
/// digits that read back as x, plain from 10⁻⁶ to below 10²¹, else in
/// exponent form. -0 reads `0`, as in JavaScript.
pub fn to_string(x: f64, out: &mut String) {
    if x == 0.0 {
        out.push('0');
        return;
    }
    if x < 0.0 {
        out.push('-');
    }
    let (digits, n) = shortest_digits(x.abs());
    let len = digits.len() as i32;
    if len <= n && n <= 21 {
        push_digits(&digits, out);
        push_zeros((n - len) as usize, out);
    } else if 0 < n && n <= 21 {
        push_digits(&digits[..n as usize], out);
        out.push('.');
        push_digits(&digits[n as usize..], out);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        push_zeros((-n) as usize, out);
        push_digits(&digits, out);
    } else {
        out.push(char::from(b'0' + digits[0]));
        if len > 1 {
            out.push('.');
            push_digits(&digits[1..], out);
        }
        push_exponent(n - 1, out);
    }
}

/// The digits of a finite x ≥ 10²¹ written out in full from its shortest
/// digits and zeros, as `x.toLocaleString("en")` gives them without the
/// group separators (d3-format's `formatDecimal` for huge integers).
pub fn to_integer_string(x: f64, out: &mut String) {
    let (digits, n) = shortest_digits(x);
    push_digits(&digits, out);
    push_zeros((n - digits.len() as i32).max(0) as usize, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(f: impl Fn(&mut String)) -> String {
        let mut out = String::new();
        f(&mut out);
        out
    }

    #[test]
    fn math_round_sends_halves_up() {
        assert_eq!(round(2.5), 3.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(0.49999999999999994), 0.0);
        assert_eq!(round(4_503_599_627_370_497.0), 4_503_599_627_370_497.0);
        assert!(round(-0.5).is_sign_negative() && round(-0.5) == 0.0);
        assert!(round(-0.2).is_sign_negative());
        assert!(!round(0.2).is_sign_negative());
    }

    #[test]
    fn math_max_and_min_order_zeros_and_nan() {
        assert!(max(0.0, -0.0).is_sign_positive());
        assert!(max(-0.0, 0.0).is_sign_positive());
        assert!(min(0.0, -0.0).is_sign_negative());
        assert!(max(1.0, f64::NAN).is_nan());
        assert_eq!(max(1.0, 2.0), 2.0);
    }

    #[test]
    fn pow10_is_correctly_rounded() {
        for n in -330..=310 {
            let want: f64 = if n > 308 { f64::INFINITY } else { ("1e".to_string() + &n.to_string()).parse().unwrap() };
            assert_eq!(pow10(f64::from(n)).to_bits(), want.to_bits(), "10^{n}");
        }
        assert_eq!(pow10(f64::INFINITY), f64::INFINITY);
        assert_eq!(pow10(f64::NEG_INFINITY), 0.0);
        assert!(pow10(f64::NAN).is_nan());
    }

    /// Doubles where macOS's `log10` and V8's (Node 26) disagree about the
    /// floor, captured with `Math.floor(Math.log10(x))`.
    #[test]
    fn log10_matches_node_near_powers_of_ten() {
        for (x, want) in [(9.999999999999345e-304, -303.0), (9.999999999999345e-301, -300.0), (9.999999999999346e-299, -298.0)] {
            assert_eq!(floor_log10(x), want, "{x:e}");
        }
        assert_eq!(floor_log10(99.99999999999999), 2.0);
        assert_eq!(floor_log10(9.999999999999998), 0.0);
        assert_eq!(floor_log10(1000.0), 3.0);
        assert_eq!(floor_log10(0.001), -3.0);
        assert_eq!(floor_log10(0.0), f64::NEG_INFINITY);
        assert!(floor_log10(-1.0).is_nan());
    }

    #[test]
    fn conversions_match_javascript() {
        assert_eq!(s(|o| to_fixed(1.005, 2, o)), "1.00");
        assert_eq!(s(|o| to_fixed(1.5, 0, o)), "2");
        assert_eq!(s(|o| to_fixed(0.5, 0, o)), "1");
        assert_eq!(s(|o| to_fixed(0.05, 1, o)), "0.1");
        assert_eq!(s(|o| to_fixed(0.0004, 3, o)), "0.000");
        assert_eq!(s(|o| to_fixed(0.0, 2, o)), "0.00");
        assert_eq!(s(|o| to_fixed(123.456, 1, o)), "123.5");
        assert_eq!(s(|o| to_fixed(1e21, 2, o)), "1e+21");
        assert_eq!(s(|o| to_fixed(999.99, 1, o)), "1000.0");
        assert_eq!(s(|o| to_exponential(123456.0, Some(2), o)), "1.23e+5");
        assert_eq!(s(|o| to_exponential(0.00015, None, o)), "1.5e-4");
        assert_eq!(s(|o| to_exponential(0.0, Some(1), o)), "0.0e+0");
        assert_eq!(s(|o| to_exponential(9.99, Some(1), o)), "1.0e+1");
        assert_eq!(s(|o| to_precision(123.456, 4, o)), "123.5");
        assert_eq!(s(|o| to_precision(0.000001234, 2, o)), "0.0000012");
        assert_eq!(s(|o| to_precision(1.234e-7, 2, o)), "1.2e-7");
        assert_eq!(s(|o| to_precision(123456.0, 2, o)), "1.2e+5");
        assert_eq!(s(|o| to_precision(0.0, 3, o)), "0.00");
        assert_eq!(s(|o| to_string(0.1 + 0.2, o)), "0.30000000000000004");
        assert_eq!(s(|o| to_string(1e21, o)), "1e+21");
        assert_eq!(s(|o| to_string(123e20, o)), "1.23e+22");
        assert_eq!(s(|o| to_string(123e18, o)), "123000000000000000000");
        assert_eq!(s(|o| to_string(1e-7, o)), "1e-7");
        assert_eq!(s(|o| to_string(0.000001, o)), "0.000001");
        assert_eq!(s(|o| to_string(5e-324, o)), "5e-324");
        assert_eq!(s(|o| to_string(f64::MAX, o)), "1.7976931348623157e+308");
        assert_eq!(s(|o| to_string(-2.5, o)), "-2.5");
        assert_eq!(s(|o| to_string(9007199254740993.0, o)), "9007199254740992");
        assert_eq!(s(|o| to_integer_string(2f64.powi(80), o)), "1208925819614629200000000");
        assert_eq!(exponent(1e23), Some(23));
        assert_eq!(exponent(0.0), None);
    }
}
