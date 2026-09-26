//! Build automation for the rusty-skews workspace.

mod bench_live;

use anyhow::Result;
use clap::{Parser, Subcommand};
use xshell::{Shell, cmd};

/// Build automation for the rusty-skews workspace.
#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Build automation for rusty-skews")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

/// Available automation commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Run the full local CI gate (fmt, clippy, test, doc, deny, audit).
    Ci,
    /// Check formatting.
    Fmt,
    /// Run clippy with warnings denied.
    Clippy,
    /// Run the test suite (nextest when available).
    Test,
    /// Build documentation with warnings denied.
    Doc,
    /// Run cargo-deny when installed.
    Deny,
    /// Run cargo-audit when installed.
    Audit,
    /// Run the shell briefly as a smoke demo.
    Demo {
        /// How long to run the shell before it exits by itself.
        #[arg(long, default_value_t = 5)]
        seconds: u64,
    },
    /// Measure the live budgets (cold start, idle CPU, RSS) of the release shell.
    BenchLive {
        /// Measurement window in seconds (use 900+ for a soak run).
        #[arg(long, default_value_t = 60)]
        seconds: u64,
        /// Skip the release build (assume the binary is current).
        #[arg(long)]
        skip_build: bool,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    let sh = Shell::new()?;
    sh.change_dir(workspace_root());

    match args.command.unwrap_or(Command::Ci) {
        Command::Fmt => fmt(&sh),
        Command::Clippy => clippy(&sh),
        Command::Test => test(&sh),
        Command::Doc => doc(&sh),
        Command::Deny => deny(&sh),
        Command::Audit => audit(&sh),
        Command::Demo { seconds } => demo(&sh, seconds),
        Command::BenchLive {
            seconds,
            skip_build,
        } => bench_live(&sh, seconds, skip_build),
        Command::Ci => ci(&sh),
    }
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

fn fmt(sh: &Shell) -> Result<()> {
    eprintln!("==> cargo fmt --check");
    cmd!(sh, "cargo fmt --all --check").run()?;
    Ok(())
}

fn clippy(sh: &Shell) -> Result<()> {
    eprintln!("==> cargo clippy");
    cmd!(
        sh,
        "cargo clippy --workspace --all-targets --all-features -- -D warnings"
    )
    .run()?;
    Ok(())
}

fn test(sh: &Shell) -> Result<()> {
    if is_installed("cargo-nextest") {
        eprintln!("==> cargo nextest run");
        cmd!(sh, "cargo nextest run --workspace --all-features").run()?;
    } else {
        eprintln!("==> cargo test (nextest not installed)");
        cmd!(sh, "cargo test --workspace --all-features").run()?;
    }
    Ok(())
}

fn doc(sh: &Shell) -> Result<()> {
    eprintln!("==> cargo doc");
    cmd!(sh, "cargo doc --workspace --no-deps --all-features")
        .env("RUSTDOCFLAGS", "-D warnings")
        .run()?;
    Ok(())
}

fn deny(sh: &Shell) -> Result<()> {
    if is_installed("cargo-deny") {
        eprintln!("==> cargo deny");
        cmd!(sh, "cargo deny check").run()?;
    } else {
        eprintln!(
            "==> cargo-deny not installed; skipping (install with `cargo install cargo-deny`)"
        );
    }
    Ok(())
}

fn audit(sh: &Shell) -> Result<()> {
    if is_installed("cargo-audit") {
        eprintln!("==> cargo audit");
        cmd!(sh, "cargo audit").run()?;
    } else {
        eprintln!(
            "==> cargo-audit not installed; skipping (install with `cargo install cargo-audit`)"
        );
    }
    Ok(())
}

/// Returns `true` when `program` is available on `PATH`.
fn is_installed(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

fn demo(sh: &Shell, seconds: u64) -> Result<()> {
    eprintln!("==> rusty-skews demo for {seconds}s (requires a running Wayland session)");
    let seconds = seconds.to_string();
    cmd!(sh, "cargo run -q -p skews-shell -- --exit-after {seconds}").run()?;
    Ok(())
}

/// Builds the release shell and measures the live budgets.
fn bench_live(sh: &Shell, seconds: u64, skip_build: bool) -> Result<()> {
    if !skip_build {
        eprintln!("==> cargo build --release -p skews-shell");
        cmd!(sh, "cargo build --release -p skews-shell").run()?;
    }

    let binary = workspace_root().join("target/release/rusty-skews");
    anyhow::ensure!(
        binary.exists(),
        "release binary missing at {}; run without --skip-build",
        binary.display()
    );

    eprintln!("==> measuring for {seconds}s (requires a running Wayland session)");
    let log = bench_live::default_log_path();
    let report = bench_live::run(&binary, seconds, &log)?;
    print!("{}", report.render());
    eprintln!("log: {}", log.display());

    anyhow::ensure!(report.all_budgets_pass(), "live budgets not met");

    Ok(())
}

fn ci(sh: &Shell) -> Result<()> {
    fmt(sh)?;
    clippy(sh)?;
    test(sh)?;
    doc(sh)?;
    deny(sh)?;
    audit(sh)?;
    eprintln!("==> CI gate passed");
    Ok(())
}
