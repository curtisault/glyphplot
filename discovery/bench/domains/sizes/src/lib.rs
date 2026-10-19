//! Throwaway size probe: an astronomy pack (Keplerian elements, RA/Dec, alt/az)
//! and an indicator pack (SMA, EMA, Bollinger, RSI, MACD), each behind a feature.
#![no_std]

#[cfg(not(test))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(feature = "astro")]
pub mod astro {
    use libm::{asin, atan2, cos, fmod, sin, sqrt};
    const D: f64 = core::f64::consts::PI / 180.0;
    /// JPL approx_pos Table 1: a, e, I, L, varpi, Omega, then their rates per century.
    static EL: [[f64; 12]; 8] = [
        [0.38709927, 0.20563593, 7.00497902, 252.25032350, 77.45779628, 48.33076593, 0.00000037, 0.00001906, -0.00594749, 149472.67411175, 0.16047689, -0.12534081],
        [0.72333566, 0.00677672, 3.39467605, 181.97909950, 131.60246718, 76.67984255, 0.00000390, -0.00004107, -0.00078890, 58517.81538729, 0.00268329, -0.27769418],
        [1.00000261, 0.01671123, -0.00001531, 100.46457166, 102.93768193, 0.0, 0.00000562, -0.00004392, -0.01294668, 35999.37244981, 0.32327364, 0.0],
        [1.52371034, 0.09339410, 1.84969142, -4.55343205, -23.94362959, 49.55953891, 0.00001847, 0.00007882, -0.00813131, 19140.30268499, 0.44441088, -0.29257343],
        [5.20288700, 0.04838624, 1.30439695, 34.39644051, 14.72847983, 100.47390909, -0.00011607, -0.00013253, -0.00183714, 3034.74612775, 0.21252668, 0.20469106],
        [9.53667594, 0.05386179, 2.48599187, 49.95424423, 92.59887831, 113.66242448, -0.00125060, -0.00050991, 0.00193609, 1222.49362201, -0.41897216, -0.28867794],
        [19.18916464, 0.04725744, 0.77263783, 313.23810451, 170.95427630, 74.01692503, -0.00196176, -0.00004397, -0.00242939, 428.48202785, 0.40805281, 0.04240589],
        [30.06992276, 0.00859048, 1.77004347, -55.12002969, 44.96476227, 131.78422574, 0.00026291, 0.00005105, 0.00035372, 218.45945325, -0.32241464, -0.00508664],
    ];
    fn helio(p: usize, jd: f64) -> [f64; 3] {
        let t = (jd - 2451545.0) / 36525.0;
        let e = &EL[p];
        let k = |i: usize| e[i] + e[i + 6] * t;
        let (a, ec, i, l, wb, o) = (k(0), k(1), k(2) * D, k(3), k(4), k(5));
        let w = (wb - o) * D;
        let m = (fmod(fmod(l - wb + 180.0, 360.0) + 360.0, 360.0) - 180.0) * D;
        let mut ea = m + ec * sin(m);
        for _ in 0..8 {
            ea -= (ea - ec * sin(ea) - m) / (1.0 - ec * cos(ea));
        }
        let (xp, yp) = (a * (cos(ea) - ec), a * sqrt(1.0 - ec * ec) * sin(ea));
        let o = o * D;
        let (cw, sw, co, so, ci, si) = (cos(w), sin(w), cos(o), sin(o), cos(i), sin(i));
        [
            (cw * co - sw * so * ci) * xp + (-sw * co - cw * so * ci) * yp,
            (cw * so + sw * co * ci) * xp + (-sw * so + cw * co * ci) * yp,
            sw * si * xp + cw * si * yp,
        ]
    }
    /// Geocentric RA, Dec (degrees) of planet `p` at Julian date `jd`, then alt/az for a site.
    #[unsafe(no_mangle)]
    pub extern "C" fn planet_radec_altaz(p: u32, jd: f64, lat: f64, lon: f64, out: *mut f64) {
        let (h, g) = (helio(p as usize % 8, jd), helio(2, jd));
        let (x, y0, z0) = (h[0] - g[0], h[1] - g[1], h[2] - g[2]);
        let eps = 23.43928 * D;
        let (y, z) = (y0 * cos(eps) - z0 * sin(eps), y0 * sin(eps) + z0 * cos(eps));
        let ra = atan2(y, x);
        let dec = asin(z / sqrt(x * x + y * y + z * z));
        let gmst = (280.46061837 + 360.98564736629 * (jd - 2451545.0)) * D;
        let ha = gmst + lon * D - ra;
        let la = lat * D;
        let alt = asin(sin(la) * sin(dec) + cos(la) * cos(dec) * cos(ha));
        let az = atan2(-cos(dec) * sin(ha), sin(dec) * cos(la) - cos(dec) * sin(la) * cos(ha));
        let r = [ra / D, dec / D, alt / D, az / D];
        for (k, v) in r.iter().enumerate() {
            unsafe { *out.add(k) = *v };
        }
    }
}

#[cfg(feature = "indicators")]
pub mod indicators {
    use libm::sqrt;
    fn s<'a>(p: *const f64, n: usize) -> &'a [f64] {
        unsafe { core::slice::from_raw_parts(p, n) }
    }
    fn m<'a>(p: *mut f64, n: usize) -> &'a mut [f64] {
        unsafe { core::slice::from_raw_parts_mut(p, n) }
    }
    /// Simple moving average over `w` samples; NaN until the window fills.
    #[unsafe(no_mangle)]
    pub extern "C" fn sma(x: *const f64, n: usize, w: usize, out: *mut f64) {
        let (x, o) = (s(x, n), m(out, n));
        let mut acc = 0.0;
        for i in 0..n {
            acc += x[i];
            if i >= w { acc -= x[i - w]; }
            o[i] = if i + 1 >= w { acc / w as f64 } else { f64::NAN };
        }
    }
    /// Exponential moving average with alpha 2/(w+1), seeded by the first sample.
    #[unsafe(no_mangle)]
    pub extern "C" fn ema(x: *const f64, n: usize, w: usize, out: *mut f64) {
        let (x, o) = (s(x, n), m(out, n));
        let a = 2.0 / (w as f64 + 1.0);
        let mut e = f64::NAN;
        for i in 0..n {
            e = if e.is_nan() { x[i] } else { e + a * (x[i] - e) };
            o[i] = e;
        }
    }
    /// Bollinger bands: middle, upper, lower (k standard deviations, population).
    #[unsafe(no_mangle)]
    pub extern "C" fn bollinger(x: *const f64, n: usize, w: usize, k: f64, mid: *mut f64, up: *mut f64, lo: *mut f64) {
        let (x, mi, u, l) = (s(x, n), m(mid, n), m(up, n), m(lo, n));
        let (mut a, mut q) = (0.0, 0.0);
        for i in 0..n {
            a += x[i];
            q += x[i] * x[i];
            if i >= w { a -= x[i - w]; q -= x[i - w] * x[i - w]; }
            if i + 1 >= w {
                let mean = a / w as f64;
                let sd = sqrt((q / w as f64 - mean * mean).max(0.0));
                (mi[i], u[i], l[i]) = (mean, mean + k * sd, mean - k * sd);
            } else {
                (mi[i], u[i], l[i]) = (f64::NAN, f64::NAN, f64::NAN);
            }
        }
    }
    /// Wilder's RSI over `w` periods.
    #[unsafe(no_mangle)]
    pub extern "C" fn rsi(x: *const f64, n: usize, w: usize, out: *mut f64) {
        let (x, o) = (s(x, n), m(out, n));
        let (mut g, mut d) = (0.0, 0.0);
        let wf = w as f64;
        for i in 0..n {
            if i == 0 { o[0] = f64::NAN; continue; }
            let c = x[i] - x[i - 1];
            let (up, dn) = (c.max(0.0), (-c).max(0.0));
            if i <= w { g += up / wf; d += dn / wf; } else { g = (g * (wf - 1.0) + up) / wf; d = (d * (wf - 1.0) + dn) / wf; }
            o[i] = if i < w { f64::NAN } else if d == 0.0 { 100.0 } else { 100.0 - 100.0 / (1.0 + g / d) };
        }
    }
    /// MACD line, signal and histogram (fast, slow, signal periods).
    #[unsafe(no_mangle)]
    pub extern "C" fn macd(x: *const f64, n: usize, f: usize, sl: usize, sg: usize, line: *mut f64, sig: *mut f64, hist: *mut f64) {
        let (x, li, si, hi) = (s(x, n), m(line, n), m(sig, n), m(hist, n));
        let (af, asl, asg) = (2.0 / (f as f64 + 1.0), 2.0 / (sl as f64 + 1.0), 2.0 / (sg as f64 + 1.0));
        let (mut ef, mut es, mut eg) = (x.first().copied().unwrap_or(0.0), x.first().copied().unwrap_or(0.0), 0.0);
        for i in 0..n {
            ef += af * (x[i] - ef);
            es += asl * (x[i] - es);
            li[i] = ef - es;
            eg = if i == 0 { li[0] } else { eg + asg * (li[i] - eg) };
            si[i] = eg;
            hi[i] = li[i] - eg;
        }
    }
}
