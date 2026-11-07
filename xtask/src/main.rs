//! Project tasks.
//!
//! ```text
//! cargo xtask bench [--baseline FILE] [--out FILE] [--summary FILE] [--runs N]
//! ```
//!
//! `bench` builds the release binaries, measures the metrics in
//! `bench/budgets.toml`, and checks them against the budgets and the baseline.
//! It exits with status 1 if a check fails.

mod bench;

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("bench") => match bench::Options::parse(args).and_then(bench::run) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("usage: cargo xtask bench [--baseline FILE] [--out FILE] [--summary FILE] [--runs N]");
            ExitCode::from(2)
        }
    }
}
