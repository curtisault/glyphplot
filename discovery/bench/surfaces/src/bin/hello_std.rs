//! Throwaway: the hello chart printed as text with std only (the floor for the CLI sizes).
use std::io::Read;

fn main() {
    let mut s = String::new();
    let values = if std::io::stdin().read_to_string(&mut s).is_ok() && !s.trim().is_empty() { bench_surfaces::parse(&s) } else { bench_surfaces::demo(200) };
    print!("{}", bench_surfaces::chart(&values, 80, 24, None).text());
}
