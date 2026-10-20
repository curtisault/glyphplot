"""Compare JPL's approximate Keplerian elements (Table 1, 1800-2050) with Horizons
vectors for 2026. Throwaway. Usage: python3 -I kepler_vs_horizons.py <dir with b399/b499/b599_2026.json>"""
import json, math, sys, os

# Table 1 of https://ssd.jpl.nasa.gov/planets/approx_pos.html: a, e, I, L, varpi, Omega and rates per century.
EL = {
    399: ([1.00000261, 0.01671123, -0.00001531, 100.46457166, 102.93768193, 0.0],
          [0.00000562, -0.00004392, -0.01294668, 35999.37244981, 0.32327364, 0.0]),
    499: ([1.52371034, 0.09339410, 1.84969142, -4.55343205, -23.94362959, 49.55953891],
          [0.00001847, 0.00007882, -0.00813131, 19140.30268499, 0.44441088, -0.29257343]),
    599: ([5.20288700, 0.04838624, 1.30439695, 34.39644051, 14.72847983, 100.47390909],
          [-0.00011607, -0.00013253, -0.00183714, 3034.74612775, 0.21252668, 0.20469106]),
}
AU = 149597870.7  # km
OBL = math.radians(23.43928)

def kepler(body, jd):
    T = (jd - 2451545.0) / 36525.0
    a, e, I, L, wb, O = [x + r * T for x, r in zip(*EL[body])]
    w = wb - O
    M = ((L - wb + 180.0) % 360.0) - 180.0
    Mr, er = math.radians(M), e
    E = Mr + er * math.sin(Mr)
    for _ in range(10):
        E -= (E - er * math.sin(E) - Mr) / (1 - er * math.cos(E))
    xp, yp = a * (math.cos(E) - e), a * math.sqrt(1 - e * e) * math.sin(E)
    w, O, I = map(math.radians, (w, O, I))
    cw, sw, cO, sO, cI, sI = math.cos(w), math.sin(w), math.cos(O), math.sin(O), math.cos(I), math.sin(I)
    x = (cw * cO - sw * sO * cI) * xp + (-sw * cO - cw * sO * cI) * yp
    y = (cw * sO + sw * cO * cI) * xp + (-sw * sO + cw * cO * cI) * yp
    z = (sw * sI) * xp + (cw * sI) * yp
    return (x * AU, y * AU, z * AU)

def horizons(path):
    r = json.load(open(path))["result"].split("$$SOE")[1].split("$$EOE")[0]
    out = []
    for line in r.strip().splitlines():
        f = [s.strip() for s in line.split(",")]
        out.append((float(f[0]), (float(f[2]), float(f[3]), float(f[4]))))
    return out

def ang(u, v):
    d = sum(a * b for a, b in zip(u, v)) / (math.hypot(*u) * math.hypot(*v))
    return math.degrees(math.acos(max(-1.0, min(1.0, d)))) * 3600.0

def radec(v):
    x, y, z = v
    y, z = y * math.cos(OBL) - z * math.sin(OBL), y * math.sin(OBL) + z * math.cos(OBL)
    return math.degrees(math.atan2(y, x)) % 360.0, math.degrees(math.asin(z / math.hypot(x, y, z)))

d = sys.argv[1]
H = {b: horizons(os.path.join(d, f"b{b}_2026.json")) for b in EL}
for b in (499, 599):
    helio, geo = [], []
    for (jd, hv), (_, ev) in zip(H[b], H[399]):
        kv, ke = kepler(b, jd), kepler(399, jd)
        helio.append(ang(kv, hv))
        geo.append(ang(tuple(p - q for p, q in zip(kv, ke)), tuple(p - q for p, q in zip(hv, ev))))
    print(f"body {b}: days {len(helio)}  heliocentric max {max(helio):.0f}\" mean {sum(helio)/len(helio):.0f}\"  "
          f"geocentric max {max(geo):.0f}\" ({max(geo)/60:.1f}') mean {sum(geo)/len(geo):.0f}\"")
print("example geocentric RA/Dec of Mars 2026-01-01 (Horizons geometric):",
      tuple(round(x, 3) for x in radec(tuple(p - q for p, q in zip(H[499][0][1], H[399][0][1])))))
