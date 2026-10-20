//! The binary's usage and version.

use std::process::Command;

fn glyphplot(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_glyphplot")).args(args).output().expect("runs the binary")
}

#[test]
fn version_prints_the_crate_version() {
    let out = glyphplot(&["--version"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), format!("glyphplot {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_names_every_family() {
    let out = glyphplot(&["--help"]);
    assert!(out.status.success());
    let families: Vec<&str> = glyphplot::FAMILIES.iter().map(|f| f.name).collect();
    assert_eq!(families, ["line", "candles", "bar"], "the default build registers the three families");
    assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("Families: {}\n", families.join(", "))));
}

#[test]
fn an_unknown_argument_prints_usage_to_stderr_and_exits_2() {
    let out = glyphplot(&["frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("glyphplot "));
}
