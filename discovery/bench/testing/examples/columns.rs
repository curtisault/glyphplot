//! Counts the points that two algebraically equal column mappings put in
//! different columns: x * cols / n against x * (cols / n), for n points.
fn main() {
    let (mut cases, mut differ, mut first) = (0u64, 0u64, None);
    for n in 1..=2000usize {
        for cols in [40usize, 80, 160, 316, 320, 640] {
            let x1 = n as f64;
            for i in 0..n {
                let x = i as f64;
                let a = ((x * cols as f64 / x1) as usize).min(cols - 1);
                let b = ((x * (cols as f64 / x1)) as usize).min(cols - 1);
                cases += 1;
                if a != b {
                    differ += 1;
                    first.get_or_insert((x, n, cols, a, b));
                }
            }
        }
    }
    println!("{differ} of {cases} points land in another column; first: {first:?} (x, n, cols, x*cols/n, x*(cols/n))");
}
