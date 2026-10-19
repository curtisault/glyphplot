//! `Math.log10` as V8 14.6 computes it (Node 26), so `d3-array`'s
//! `Math.floor(Math.log10(step))` gives the same power of ten here, natively
//! and in wasm, as in the browser (REQ-TICK-010). The platform's `log10`
//! does not: macOS's differed from Node 26.5.1 (arm64) on 6,693 of 2,889,524
//! doubles (each power of ten with the 3,005 doubles below it and 5 above,
//! and 1,000,000 random doubles), and on 129 of them in the floor.
//!
//! V8's arm64 build fuses `a * b + c` into one fused multiply-add where the
//! C++ writes it in one expression (clang's `-ffp-contract=on`, the left
//! product first), so this port does the same with `f64::mul_add`; it then
//! matched Node 26.5.1 on all 2,889,524 doubles. Without the fusions it
//! differed on 226, 5 of them in the floor, all beyond 10^±166; x86-64 builds
//! of V8, which do not fuse, give those.
//!
//! Ported from V8's `src/base/ieee754.cc` (tag 14.6.202.34, functions `log`
//! and `log10`), which carries this notice:
//!
//! ```text
//! The following is adapted from fdlibm (http://www.netlib.org/fdlibm).
//!
//! ====================================================
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//!
//! Developed at SunSoft, a Sun Microsystems, Inc. business.
//! Permission to use, copy, modify, and distribute this
//! software is freely granted, provided that this notice
//! is preserved.
//! ====================================================
//!
//! The original source code covered by the above license above has been
//! modified significantly by Google Inc.
//! Copyright 2016 the V8 project authors. All rights reserved.
//! ```

// The constants are fdlibm's, digit for digit as published.
#![allow(clippy::excessive_precision, clippy::approx_constant)]

fn high(x: f64) -> i32 {
    (x.to_bits() >> 32) as u32 as i32
}

fn low(x: f64) -> u32 {
    x.to_bits() as u32
}

fn with_high(x: f64, hx: i32) -> f64 {
    f64::from_bits((u64::from(hx as u32) << 32) | u64::from(low(x)))
}

const TWO54: f64 = 1.80143985094819840000e+16;

/// fdlibm's `log(x)`, the natural logarithm, under 1 ulp of error.
pub(crate) fn log(mut x: f64) -> f64 {
    const LN2_HI: f64 = 6.93147180369123816490e-01;
    const LN2_LO: f64 = 1.90821492927058770002e-10;
    const LG1: f64 = 6.666666666666735130e-01;
    const LG2: f64 = 3.999999999940941908e-01;
    const LG3: f64 = 2.857142874366239149e-01;
    const LG4: f64 = 2.222219843214978396e-01;
    const LG5: f64 = 1.818357216161805012e-01;
    const LG6: f64 = 1.531383769920937332e-01;
    const LG7: f64 = 1.479819860511658591e-01;

    let mut hx = high(x);
    let lx = low(x);
    let mut k = 0i32;
    if hx < 0x0010_0000 {
        if ((hx & 0x7FFF_FFFF) as u32 | lx) == 0 {
            return f64::NEG_INFINITY;
        }
        if hx < 0 {
            return f64::NAN;
        }
        k -= 54;
        x *= TWO54;
        hx = high(x);
    }
    if hx >= 0x7FF0_0000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    hx &= 0x000F_FFFF;
    let i = (hx + 0x95F64) & 0x10_0000;
    x = with_high(x, hx | (i ^ 0x3FF0_0000));
    k += i >> 20;
    let f = x - 1.0;
    if (0x000F_FFFF & (2 + hx)) < 3 {
        if f == 0.0 {
            if k == 0 {
                return 0.0;
            }
            let dk = f64::from(k);
            return dk.mul_add(LN2_HI, dk * LN2_LO);
        }
        let r = f * f * (-0.33333333333333333f64).mul_add(f, 0.5);
        if k == 0 {
            return f - r;
        }
        let dk = f64::from(k);
        return dk.mul_add(LN2_HI, -((-dk).mul_add(LN2_LO, r) - f));
    }
    let s = f / (2.0 + f);
    let dk = f64::from(k);
    let z = s * s;
    let mut i = hx - 0x6147A;
    let w = z * z;
    let j = 0x6B851 - hx;
    let t1 = w * w.mul_add(w.mul_add(LG6, LG4), LG2);
    let t2 = z * w.mul_add(w.mul_add(w.mul_add(LG7, LG5), LG3), LG1);
    i |= j;
    let r = t2 + t1;
    if i > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            f - (-s).mul_add(hfsq + r, hfsq)
        } else {
            dk.mul_add(LN2_HI, -((hfsq - s.mul_add(hfsq + r, dk * LN2_LO)) - f))
        }
    } else if k == 0 {
        (-s).mul_add(f - r, f)
    } else {
        dk.mul_add(LN2_HI, -(s.mul_add(f - r, -(dk * LN2_LO)) - f))
    }
}

/// fdlibm's `log10(x)`, as V8's `Math.log10`; exact on 10ⁿ for n = 0 to 22.
pub(crate) fn log10(mut x: f64) -> f64 {
    const IVLN10: f64 = 4.34294481903251816668e-01;
    const LOG10_2HI: f64 = 3.01029995663611771306e-01;
    const LOG10_2LO: f64 = 3.69423907715893078616e-13;

    let mut hx = high(x);
    let mut lx = low(x);
    let mut k = 0i32;
    if hx < 0x0010_0000 {
        if ((hx & 0x7FFF_FFFF) as u32 | lx) == 0 {
            return f64::NEG_INFINITY;
        }
        if hx < 0 {
            return f64::NAN;
        }
        k -= 54;
        x *= TWO54;
        hx = high(x);
        lx = low(x);
    }
    if hx >= 0x7FF0_0000 {
        return x + x;
    }
    if hx == 0x3FF0_0000 && lx == 0 {
        return 0.0;
    }
    k += (hx >> 20) - 1023;
    let i = ((k as u32 & 0x8000_0000) >> 31) as i32;
    hx = (hx & 0x000F_FFFF) | ((0x3FF - i) << 20);
    let y = f64::from(k + i);
    x = f64::from_bits((u64::from(hx as u32) << 32) | u64::from(lx));
    let z = y.mul_add(LOG10_2LO, IVLN10 * log(x));
    y.mul_add(LOG10_2HI, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_on_powers_of_ten() {
        let mut p = 1.0;
        for n in 0..=22 {
            assert_eq!(log10(p), f64::from(n));
            p *= 10.0;
        }
        assert_eq!(log10(0.0), f64::NEG_INFINITY);
        assert!(log10(-1.0).is_nan());
        assert_eq!(log10(f64::INFINITY), f64::INFINITY);
    }
}
