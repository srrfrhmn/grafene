//! `grf-server`: the Grafene server.
//!
//! M0 gives only `--version` and `--help`, so that CI can measure the binary.
//! The axum app comes in M3.

use clap::Parser;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Grafene server.
#[derive(Debug, Parser)]
#[command(name = "grf-server", version, about, arg_required_else_help = true)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
