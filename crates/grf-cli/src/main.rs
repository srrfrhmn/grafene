//! `grf`: the Grafene command-line tool.
//!
//! M0 gives only `--version` and `--help`. The stack commands come in M1
//! (design section 7.3).

use clap::Parser;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Stacked pull requests and code review.
#[derive(Debug, Parser)]
#[command(name = "grf", version, about, arg_required_else_help = true)]
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
