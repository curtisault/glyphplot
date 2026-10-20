//! A small unsigned big integer, with just the exact arithmetic that
//! JavaScript's number-to-string conversions and `Math.log10` need: an `f64`
//! is `m × 2^e`, and its decimal digits and its distance from a power of ten
//! are ratios of such integers (REQ-FMT-030).

use core::cmp::Ordering;

/// An unsigned integer of any size, little-endian base-2³² limbs with no
/// zero limb on top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Big {
    limbs: Vec<u32>,
}

impl Big {
    /// The integer `v`.
    pub(crate) fn from_u128(mut v: u128) -> Big {
        let mut limbs = Vec::with_capacity(4);
        while v != 0 {
            limbs.push(v as u32);
            v >>= 32;
        }
        Big { limbs }
    }

    /// The integer `v`.
    pub(crate) fn from_u64(v: u64) -> Big {
        Big::from_u128(u128::from(v))
    }

    /// 10ⁿ.
    pub(crate) fn pow10(n: u32) -> Big {
        let mut b = Big::from_u64(1);
        b.mul_pow10(n);
        b
    }

    /// Whether the integer is 0.
    pub(crate) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    /// The number of bits up to the highest set bit; 0 for 0.
    pub(crate) fn bit_len(&self) -> u32 {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() as u32 - 1) * 32 + (32 - top.leading_zeros()),
        }
    }

    /// Multiplies by a small factor in place.
    pub(crate) fn mul_small(&mut self, m: u32) {
        let mut carry = 0u64;
        for limb in &mut self.limbs {
            let p = u64::from(*limb) * u64::from(m) + carry;
            *limb = p as u32;
            carry = p >> 32;
        }
        if carry != 0 {
            self.limbs.push(carry as u32);
        }
        self.trim();
    }

    /// Multiplies by 10ⁿ in place.
    pub(crate) fn mul_pow10(&mut self, mut n: u32) {
        while n >= 9 {
            self.mul_small(1_000_000_000);
            n -= 9;
        }
        if n > 0 {
            self.mul_small(10u32.pow(n));
        }
    }

    /// Multiplies by 2ⁿ in place.
    pub(crate) fn shl(&mut self, n: u32) {
        if self.is_zero() {
            return;
        }
        let (words, bits) = ((n / 32) as usize, n % 32);
        if bits != 0 {
            let mut carry = 0u32;
            for limb in &mut self.limbs {
                let next = *limb >> (32 - bits);
                *limb = (*limb << bits) | carry;
                carry = next;
            }
            if carry != 0 {
                self.limbs.push(carry);
            }
        }
        if words != 0 {
            self.limbs.splice(0..0, core::iter::repeat_n(0, words));
        }
    }

    /// Adds `o` in place.
    pub(crate) fn add(&mut self, o: &Big) {
        if self.limbs.len() < o.limbs.len() {
            self.limbs.resize(o.limbs.len(), 0);
        }
        let mut carry = 0u64;
        for (i, limb) in self.limbs.iter_mut().enumerate() {
            let s = u64::from(*limb) + u64::from(o.limbs.get(i).copied().unwrap_or(0)) + carry;
            *limb = s as u32;
            carry = s >> 32;
        }
        if carry != 0 {
            self.limbs.push(carry as u32);
        }
    }

    /// Subtracts `o`, which must not be larger, in place.
    pub(crate) fn sub(&mut self, o: &Big) {
        debug_assert!(*self >= *o);
        let mut borrow = 0i64;
        for (i, limb) in self.limbs.iter_mut().enumerate() {
            let d = i64::from(*limb) - i64::from(o.limbs.get(i).copied().unwrap_or(0)) - borrow;
            borrow = i64::from(d < 0);
            *limb = (d + (borrow << 32)) as u32;
        }
        self.trim();
    }

    /// Replaces `self` by `self mod d` and returns `self / d`, for a quotient
    /// below 16 (a decimal digit, possibly before a carry).
    pub(crate) fn div_digit(&mut self, d: &Big) -> u8 {
        let mut q = 0u8;
        while *self >= *d && q < 16 {
            self.sub(d);
            q += 1;
        }
        q
    }

    /// The integer rounded to the nearest `f64` (ties to even), times 2ⁿ.
    pub(crate) fn to_f64_scaled(&self, n: i32) -> f64 {
        let len = self.bit_len();
        if len == 0 {
            return 0.0;
        }
        // The top 64 bits, and whether any bit below them is set.
        let shift = len.saturating_sub(64);
        let mut top = 0u64;
        for bit in (shift..len).rev() {
            top = (top << 1) | u64::from(self.bit(bit));
        }
        let sticky = (0..shift).any(|bit| self.bit(bit));
        compose(top, n + shift as i32, sticky)
    }

    fn bit(&self, i: u32) -> bool {
        self.limbs.get((i / 32) as usize).is_some_and(|l| (l >> (i % 32)) & 1 == 1)
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, o: &Big) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Big {
    fn cmp(&self, o: &Big) -> Ordering {
        self.limbs.len().cmp(&o.limbs.len()).then_with(|| self.limbs.iter().rev().cmp(o.limbs.iter().rev()))
    }
}

/// `m × 2^e`, with `sticky` saying whether bits below `m` were dropped,
/// rounded to the nearest `f64`, ties to even, through subnormals and to
/// infinity.
pub(crate) fn compose(m: u64, e: i32, sticky: bool) -> f64 {
    if m == 0 {
        return 0.0;
    }
    // Normalise so the top bit of `m` is bit 63: the value is m × 2^(e).
    let lz = m.leading_zeros();
    let m = m << lz;
    let e = e - lz as i32;
    // The value is in [2^(e+63), 2^(e+64)); its unbiased exponent is e + 63.
    let exp = e + 63;
    if exp > 1023 {
        return f64::INFINITY;
    }
    // How many low bits of `m` fall below the 53-bit significand.
    let drop = if exp >= -1022 { 11 } else { 11 + (-1022 - exp) };
    if drop > 64 {
        return 0.0;
    }
    let (kept, half, rest) = if drop == 64 {
        (0u64, m >> 63 == 1, (m << 1) != 0 || sticky)
    } else {
        let kept = m >> drop;
        let half = (m >> (drop - 1)) & 1 == 1;
        let rest = (m & ((1u64 << (drop - 1)) - 1)) != 0 || sticky;
        (kept, half, rest)
    };
    let mut kept = kept;
    if half && (rest || kept & 1 == 1) {
        kept += 1;
    }
    if exp >= -1022 {
        // Normal: `kept` has 53 bits, or 54 after a carry.
        let (kept, exp) = if kept >> 53 == 1 { (kept >> 1, exp + 1) } else { (kept, exp) };
        if exp > 1023 {
            return f64::INFINITY;
        }
        f64::from_bits(((exp + 1023) as u64) << 52 | (kept & ((1u64 << 52) - 1)))
    } else {
        // Subnormal: `kept` counts units of 2^-1074; a carry into bit 52 makes
        // the smallest normal, which the same bit pattern encodes.
        f64::from_bits(kept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_agrees_with_u128() {
        let a = Big::from_u128(0xdead_beef_1234_5678_9abc_def0);
        let b = Big::from_u64(0xffff_ffff_ffff);
        let mut c = a.clone();
        c.add(&b);
        assert_eq!(c, Big::from_u128(0xdead_beef_1234_5678_9abc_def0 + 0xffff_ffff_ffff));
        c.sub(&b);
        assert_eq!(c, a);
        let mut d = Big::from_u64(3);
        d.shl(100);
        assert_eq!(d.bit_len(), 102);
        assert_eq!(Big::pow10(30), Big::from_u128(10u128.pow(30)));
    }

    #[test]
    fn compose_rounds_to_nearest_even() {
        assert_eq!(compose(1, 0, false), 1.0);
        assert_eq!(compose(3, -1, false), 1.5);
        // 2^53 + 1 is a tie between 2^53 and 2^53 + 2: even wins.
        assert_eq!(compose((1 << 53) + 1, 0, false), 9_007_199_254_740_992.0);
        assert_eq!(compose((1 << 53) + 1, 0, true), 9_007_199_254_740_994.0);
        assert_eq!(compose(1, -1074, false), f64::from_bits(1));
        assert_eq!(compose(1, -1075, false), 0.0);
        assert_eq!(compose(1, -1075, true), f64::from_bits(1));
        assert_eq!(compose(1, 1024, false), f64::INFINITY);
        assert_eq!(Big::pow10(23).to_f64_scaled(0), 1e23);
        assert_eq!(Big::pow10(308).to_f64_scaled(0), 1e308);
    }
}
