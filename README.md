# Grafene

Stacked pull requests and AI code review, written in Rust.

Status: milestone M0 (workspace and CI). See [docs/DESIGN.md](docs/DESIGN.md).

## Build and test

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The toolchain version is in `rust-toolchain.toml`. rustup installs it automatically.

## Performance budgets

```sh
cargo xtask bench                          # measure and check budgets
cargo xtask bench --baseline old.json      # also check for regressions
```

The budgets are in `bench/budgets.toml` (design section 11).
CI runs the benchmark on each pull request and keeps the results for each commit on `main` on the `benchmarks` branch.
