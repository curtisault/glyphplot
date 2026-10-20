//! `glyphplot`: charts in the terminal. It prints its usage and version; the
//! charts, `render`, `demo` and `list` arrive with the CLI's phase
//! (REQ-CLI-010).
#![warn(missing_docs)]

use std::process::ExitCode;

fn usage() -> String {
    let families: Vec<&str> = glyphplot::FAMILIES.iter().map(|f| f.name).collect();
    format!(
        "glyphplot {}: interactive charts in Unicode glyphs\n\n\
         Usage:\n  glyphplot --version   print the version\n  glyphplot --help      print this\n\n\
         Families: {}\n",
        env!("CARGO_PKG_VERSION"),
        families.join(", ")
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--version" | "-V"] => {
            println!("glyphplot {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [] | ["--help" | "-h"] => {
            print!("{}", usage());
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{}", usage());
            ExitCode::from(2)
        }
    }
}
