//! `cargo xtask bench`: measure performance budgets (design section 11).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde::{Deserialize, Serialize};

type Error = Box<dyn std::error::Error>;

#[derive(Debug)]
pub struct Options {
    baseline: Option<PathBuf>,
    out: Option<PathBuf>,
    summary: Option<PathBuf>,
    runs: usize,
}

impl Options {
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Error> {
        let mut o = Self { baseline: None, out: None, summary: None, runs: 101 };
        while let Some(flag) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "--baseline" => o.baseline = Some(value()?.into()),
                "--out" => o.out = Some(value()?.into()),
                "--summary" => o.summary = Some(value()?.into()),
                "--runs" => o.runs = value()?.parse()?,
                _ => return Err(format!("unknown flag {flag}").into()),
            }
        }
        if o.runs == 0 {
            return Err("--runs must be 1 or more".into());
        }
        Ok(o)
    }
}

#[derive(Debug, Deserialize)]
struct Budgets {
    regression_limit: f64,
    metrics: BTreeMap<String, MetricSpec>,
}

#[derive(Debug, Deserialize)]
struct MetricSpec {
    description: String,
    unit: String,
    budget: Option<f64>,
    noise_floor: f64,
}

/// One benchmark run. CI keeps one of these for each commit on `main`.
#[derive(Debug, Serialize, Deserialize)]
struct Results {
    commit: String,
    toolchain: String,
    os: String,
    metrics: BTreeMap<String, f64>,
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Pass,
    OverBudget,
    Regression,
}

#[derive(Debug)]
struct Row<'a> {
    name: &'a str,
    spec: &'a MetricSpec,
    current: f64,
    baseline: Option<f64>,
    verdict: Verdict,
}

pub fn run(opts: Options) -> Result<bool, Error> {
    let root = workspace_root();
    let budgets: Budgets = toml::from_str(&std::fs::read_to_string(root.join("bench/budgets.toml"))?)?;

    build_release(&root)?;
    let current = measure(&root, opts.runs)?;

    for name in budgets.metrics.keys() {
        if !current.metrics.contains_key(name) {
            return Err(format!("bench/budgets.toml lists {name}, but xtask does not measure it").into());
        }
    }

    let baseline: Option<Results> = match &opts.baseline {
        Some(p) if p.exists() => Some(serde_json::from_str(&std::fs::read_to_string(p)?)?),
        Some(p) => {
            eprintln!("note: no baseline at {}; only budgets are checked", p.display());
            None
        }
        None => None,
    };

    let rows = evaluate(&budgets, baseline.as_ref(), &current);
    let table = render(&rows, baseline.as_ref(), budgets.regression_limit);
    println!("{table}");

    if let Some(p) = &opts.out {
        std::fs::write(p, serde_json::to_string_pretty(&current)? + "\n")?;
    }
    if let Some(p) = &opts.summary {
        let mut old = std::fs::read_to_string(p).unwrap_or_default();
        old.push_str(&table);
        std::fs::write(p, old)?;
    }

    Ok(rows.iter().all(|r| r.verdict == Verdict::Pass))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask is in the workspace root").to_path_buf()
}

fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from)
}

fn binary(root: &Path, name: &str) -> PathBuf {
    target_dir(root).join("release").join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn build_release(root: &Path) -> Result<(), Error> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["build", "--release", "--locked", "-p", "grf-cli", "-p", "grf-server"])
        .status()?;
    if status.success() { Ok(()) } else { Err("release build failed".into()) }
}

fn measure(root: &Path, runs: usize) -> Result<Results, Error> {
    let grf = binary(root, "grf");
    let server = binary(root, "grf-server");

    let mut metrics = BTreeMap::new();
    metrics.insert("cli_version_ms".into(), median_ms(&grf, &["--version"], runs)?);
    metrics.insert("cli_binary_bytes".into(), file_size(&grf)?);
    metrics.insert("server_binary_bytes".into(), file_size(&server)?);

    Ok(Results {
        commit: commit_id(root),
        toolchain: command_line("rustc", &["--version"], root).unwrap_or_else(|| "unknown".into()),
        os: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        metrics,
    })
}

#[allow(clippy::cast_precision_loss)]
fn file_size(path: &Path) -> Result<f64, Error> {
    let len = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len();
    Ok(len as f64)
}

/// Median wall time of `runs` executions, after 5 warm-up executions.
fn median_ms(exe: &Path, args: &[&str], runs: usize) -> Result<f64, Error> {
    let once = || -> Result<f64, Error> {
        let start = Instant::now();
        let status = Command::new(exe).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status()?;
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        if status.success() { Ok(ms) } else { Err(format!("{} failed: {status}", exe.display()).into()) }
    };
    for _ in 0..5 {
        once()?;
    }
    let mut samples = (0..runs).map(|_| once()).collect::<Result<Vec<_>, _>>()?;
    Ok(median(&mut samples))
}

fn median(samples: &mut [f64]) -> f64 {
    samples.sort_by(f64::total_cmp);
    let mid = samples.len() / 2;
    if samples.len().is_multiple_of(2) { f64::midpoint(samples[mid - 1], samples[mid]) } else { samples[mid] }
}

fn commit_id(root: &Path) -> String {
    std::env::var("GITHUB_SHA")
        .ok()
        .or_else(|| command_line("git", &["rev-parse", "HEAD"], root))
        .unwrap_or_else(|| "unknown".into())
}

fn command_line(cmd: &str, args: &[&str], dir: &Path) -> Option<String> {
    let out = Command::new(cmd).args(args).current_dir(dir).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn evaluate<'a>(budgets: &'a Budgets, baseline: Option<&Results>, current: &Results) -> Vec<Row<'a>> {
    budgets
        .metrics
        .iter()
        .map(|(name, spec)| {
            let cur = current.metrics[name];
            let base = baseline.and_then(|b| b.metrics.get(name).copied());
            let verdict = if spec.budget.is_some_and(|limit| cur > limit) {
                Verdict::OverBudget
            } else if base.is_some_and(|b| cur > b * (1.0 + budgets.regression_limit) && cur - b > spec.noise_floor) {
                Verdict::Regression
            } else {
                Verdict::Pass
            };
            Row { name, spec, current: cur, baseline: base, verdict }
        })
        .collect()
}

fn render(rows: &[Row<'_>], baseline: Option<&Results>, limit: f64) -> String {
    let mut s = String::from("## Performance budgets\n\n");
    match baseline {
        Some(b) => {
            let short = b.commit.get(..12).unwrap_or(&b.commit);
            let _ = writeln!(s, "Baseline: `{short}`. Regression limit: {:.0}%.\n", limit * 100.0);
        }
        None => s.push_str("No baseline. Only budgets are checked.\n\n"),
    }
    s.push_str("| Metric | Current | Baseline | Change | Budget | Result |\n|---|---:|---:|---:|---:|---|\n");
    for r in rows {
        let unit = &r.spec.unit;
        let change = r
            .baseline
            .filter(|b| *b != 0.0)
            .map_or("—".into(), |b| format!("{:+.1}%", (r.current / b - 1.0) * 100.0));
        let base = r.baseline.map_or("—".into(), |b| fmt_value(b, unit));
        let budget = r.spec.budget.map_or("—".into(), |b| fmt_value(b, unit));
        let result = match r.verdict {
            Verdict::Pass => "pass",
            Verdict::OverBudget => "**FAIL: over budget**",
            Verdict::Regression => "**FAIL: regression**",
        };
        let _ = writeln!(
            s,
            "| {} ({}) | {} | {base} | {change} | {budget} | {result} |",
            r.name,
            r.spec.description,
            fmt_value(r.current, unit)
        );
    }
    s
}

fn fmt_value(v: f64, unit: &str) -> String {
    match unit {
        "bytes" => format!("{:.2} MB", v / 1_000_000.0),
        "ms" => format!("{v:.2} ms"),
        _ => format!("{v} {unit}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budgets() -> Budgets {
        toml::from_str(include_str!("../../bench/budgets.toml")).unwrap()
    }

    fn results(ms: f64, cli: f64, server: f64) -> Results {
        Results {
            commit: "abc".into(),
            toolchain: "t".into(),
            os: "o".into(),
            metrics: BTreeMap::from([
                ("cli_version_ms".into(), ms),
                ("cli_binary_bytes".into(), cli),
                ("server_binary_bytes".into(), server),
            ]),
        }
    }

    fn verdicts(rows: &[Row<'_>]) -> BTreeMap<String, &'static str> {
        rows.iter()
            .map(|r| {
                let v = match r.verdict {
                    Verdict::Pass => "pass",
                    Verdict::OverBudget => "budget",
                    Verdict::Regression => "regression",
                };
                (r.name.to_owned(), v)
            })
            .collect()
    }

    #[test]
    fn budgets_file_parses() {
        let b = budgets();
        assert!((b.regression_limit - 0.10).abs() < f64::EPSILON);
        assert_eq!(b.metrics["cli_version_ms"].budget, Some(10.0));
        assert_eq!(b.metrics["cli_binary_bytes"].budget, None);
    }

    #[test]
    fn passes_without_baseline_under_budget() {
        let b = budgets();
        let rows = evaluate(&b, None, &results(2.0, 5e6, 5e6));
        assert!(rows.iter().all(|r| r.verdict == Verdict::Pass));
    }

    #[test]
    fn fails_over_budget() {
        let b = budgets();
        let v = verdicts(&evaluate(&b, None, &results(10.5, 5e6, 41e6)));
        assert_eq!(v["cli_version_ms"], "budget");
        assert_eq!(v["server_binary_bytes"], "budget");
        assert_eq!(v["cli_binary_bytes"], "pass");
    }

    #[test]
    fn size_regression_over_ten_percent_fails() {
        let b = budgets();
        let base = results(2.0, 1_000_000.0, 1_000_000.0);
        let v = verdicts(&evaluate(&b, Some(&base), &results(2.0, 1_100_001.0, 1_100_000.0)));
        assert_eq!(v["cli_binary_bytes"], "regression");
        assert_eq!(v["server_binary_bytes"], "pass", "exactly 10% is permitted");
    }

    #[test]
    fn time_regression_inside_noise_floor_passes() {
        let b = budgets();
        let base = results(2.0, 1e6, 1e6);
        // +40%, but only 0.8 ms: inside the 1 ms noise floor.
        assert_eq!(verdicts(&evaluate(&b, Some(&base), &results(2.8, 1e6, 1e6)))["cli_version_ms"], "pass");
        // +60% and 1.2 ms: a regression.
        assert_eq!(verdicts(&evaluate(&b, Some(&base), &results(3.2, 1e6, 1e6)))["cli_version_ms"], "regression");
    }

    #[test]
    fn improvement_passes() {
        let b = budgets();
        let base = results(5.0, 2e6, 2e6);
        assert!(evaluate(&b, Some(&base), &results(1.0, 1e6, 1e6)).iter().all(|r| r.verdict == Verdict::Pass));
    }

    #[test]
    fn median_odd_and_even() {
        assert!((median(&mut [3.0, 1.0, 2.0]) - 2.0).abs() < f64::EPSILON);
        assert!((median(&mut [4.0, 1.0, 3.0, 2.0]) - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn options_parse() {
        let o = Options::parse(["--runs", "7", "--out", "x.json"].map(String::from).into_iter()).unwrap();
        assert_eq!(o.runs, 7);
        assert_eq!(o.out.as_deref(), Some(Path::new("x.json")));
        assert!(Options::parse(["--runs", "0"].map(String::from).into_iter()).is_err());
        assert!(Options::parse(["--nope"].map(String::from).into_iter()).is_err());
        assert!(Options::parse(["--out"].map(String::from).into_iter()).is_err());
    }
}
