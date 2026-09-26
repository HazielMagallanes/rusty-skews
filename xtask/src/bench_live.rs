//! Live benchmark: starts the shell, waits for the first frame and samples
//! `/proc` to report the budgets from `docs/en/performance.md`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// Budgets enforced by the M1 acceptance run.
pub const BUDGET_IDLE_CPU_PERCENT: f64 = 0.2;
/// Budget for the cold start to the first bar frame.
pub const BUDGET_COLD_START_MS: u128 = 150;
/// Budget for the idle RSS.
pub const BUDGET_RSS_MIB: f64 = 60.0;
/// How often long runs print a progress line (partial data survives a crash).
pub const PROGRESS_INTERVAL: Duration = Duration::from_secs(1800);

/// A `/proc` sample of the shell process.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Resident set size in KiB.
    pub rss_kib: u64,
    /// Peak resident set size in KiB.
    pub peak_kib: u64,
    /// CPU time (user + system) in clock ticks.
    pub cpu_ticks: u64,
}

/// Results of one live benchmark run.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Milliseconds from spawn to the first rendered bar frame.
    pub cold_start_ms: u128,
    /// Average CPU usage over the measurement window, in percent of one core.
    pub idle_cpu_percent: f64,
    /// RSS at the start of the window, in KiB.
    pub rss_first_kib: u64,
    /// RSS at the end of the window, in KiB.
    pub rss_last_kib: u64,
    /// Peak RSS over the process lifetime, in KiB.
    pub rss_peak_kib: u64,
    /// Number of samples taken.
    pub samples: u64,
    /// Measurement window in seconds.
    pub window_seconds: u64,
}

impl Report {
    /// Renders the report as a human-readable table with budget verdicts.
    #[must_use]
    pub fn render(&self) -> String {
        let cpu_pass = self.idle_cpu_percent < BUDGET_IDLE_CPU_PERCENT;
        let cold_pass = self.cold_start_ms < BUDGET_COLD_START_MS;
        let rss_pass = self.rss_mib() < BUDGET_RSS_MIB;

        let verdict = |pass: bool| if pass { "PASS" } else { "FAIL" };

        format!(
            "\nrusty-skews live benchmark ({} s window)\n\
             ----------------------------------------------------------------\n\
             cold start → first frame : {:>6} ms   (budget < {} ms)    {}\n\
             idle CPU                 : {:>6.2} %   (budget < {:.1} %)    {}\n\
             RSS (end of window)      : {:>6.1} MiB (budget < {:.0} MiB)  {}\n\
             RSS (peak)               : {:>6.1} MiB\n\
             samples                  : {:>6}\n",
            self.window_seconds,
            self.cold_start_ms,
            BUDGET_COLD_START_MS,
            verdict(cold_pass),
            self.idle_cpu_percent,
            BUDGET_IDLE_CPU_PERCENT,
            verdict(cpu_pass),
            self.rss_mib(),
            BUDGET_RSS_MIB,
            verdict(rss_pass),
            self.rss_peak_kib as f64 / 1024.0,
            self.samples,
        )
    }

    /// RSS at the end of the window, in MiB.
    #[must_use]
    pub fn rss_mib(&self) -> f64 {
        self.rss_last_kib as f64 / 1024.0
    }

    /// `true` when every enforced budget passed.
    #[must_use]
    pub fn all_budgets_pass(&self) -> bool {
        self.idle_cpu_percent < BUDGET_IDLE_CPU_PERCENT
            && self.cold_start_ms < BUDGET_COLD_START_MS
            && self.rss_mib() < BUDGET_RSS_MIB
    }
}

/// Runs the shell binary and measures it for `seconds`.
pub fn run(binary: &Path, seconds: u64, log_path: &Path) -> Result<Report> {
    let log = fs::File::create(log_path)
        .with_context(|| format!("failed to create {}", log_path.display()))?;
    let log_clone = log.try_clone().context("failed to clone the log handle")?;

    let mut child = Command::new(binary)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_clone))
        .stdin(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to start {}", binary.display()))?;

    let pid = child.id();
    let cold_start = wait_for_first_frame(&mut child, log_path, Duration::from_secs(15))?;

    let mut samples = 0_u64;
    let first = sample(pid).context("failed to read the first /proc sample")?;
    let mut last = first;
    let sampling_start = Instant::now();

    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut next_progress = sampling_start + PROGRESS_INTERVAL;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(sample) = sample(pid) {
            last = sample;
            samples += 1;
        }
        if let Ok(Some(_)) = child.try_wait() {
            anyhow::bail!("the shell exited during the measurement window");
        }

        if Instant::now() >= next_progress {
            let elapsed = sampling_start.elapsed().as_secs_f64().max(1.0);
            let ticks = last.cpu_ticks.saturating_sub(first.cpu_ticks);
            let cpu = ticks as f64 / (100.0 * elapsed) * 100.0;
            eprintln!(
                "progress: elapsed={:.0}m rss={:.1} MiB peak={:.1} MiB cpu_avg={:.2}%",
                elapsed / 60.0,
                last.rss_kib as f64 / 1024.0,
                last.peak_kib as f64 / 1024.0,
                cpu,
            );
            next_progress += PROGRESS_INTERVAL;
        }
    }

    let elapsed_seconds = sampling_start.elapsed().as_secs_f64().max(1.0);
    let cpu_ticks = last.cpu_ticks.saturating_sub(first.cpu_ticks);
    // USER_HZ is 100 on every mainstream Linux configuration.
    let idle_cpu_percent = cpu_ticks as f64 / (100.0 * elapsed_seconds) * 100.0;

    terminate(&mut child);

    Ok(Report {
        cold_start_ms: cold_start.as_millis(),
        idle_cpu_percent,
        rss_first_kib: first.rss_kib,
        rss_last_kib: last.rss_kib,
        rss_peak_kib: last.peak_kib.max(first.peak_kib),
        samples,
        window_seconds: seconds,
    })
}

fn wait_for_first_frame(child: &mut Child, log_path: &Path, timeout: Duration) -> Result<Duration> {
    let start = Instant::now();

    while start.elapsed() < timeout {
        if let Ok(contents) = fs::read_to_string(log_path)
            && contents.contains("bar configured")
        {
            return Ok(start.elapsed());
        }
        if let Ok(Some(status)) = child.try_wait() {
            anyhow::bail!("the shell exited before the first frame ({status})");
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    anyhow::bail!("the shell did not render a frame within {timeout:?}");
}

fn terminate(child: &mut Child) {
    let _ = Command::new("kill")
        .arg(child.id().to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.wait();
}

/// Reads one `/proc` sample for a process.
pub fn sample(pid: u32) -> Result<Sample> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
        .with_context(|| format!("failed to read /proc/{pid}/stat"))?;
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .with_context(|| format!("failed to read /proc/{pid}/status"))?;

    let cpu_ticks = parse_cpu_ticks(&stat).context("failed to parse CPU ticks")?;
    let (rss_kib, peak_kib) = parse_status_rss(&status).context("failed to parse RSS")?;

    Ok(Sample {
        rss_kib,
        peak_kib,
        cpu_ticks,
    })
}

/// Extracts user + system CPU ticks from `/proc/<pid>/stat`.
///
/// The process name can contain spaces and parentheses, so parsing starts
/// after the last `)`.
#[must_use]
pub fn parse_cpu_ticks(stat: &str) -> Option<u64> {
    let rest = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = rest.split_whitespace().collect();

    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;

    Some(utime + stime)
}

/// Extracts `VmRSS` and `VmHWM` from `/proc/<pid>/status`, in KiB.
#[must_use]
pub fn parse_status_rss(status: &str) -> Option<(u64, u64)> {
    let mut rss = None;
    let mut peak = None;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            rss = value.split_whitespace().next()?.parse().ok();
        } else if let Some(value) = line.strip_prefix("VmHWM:") {
            peak = value.split_whitespace().next()?.parse().ok();
        }
    }

    Some((rss?, peak.unwrap_or(rss?)))
}

/// Default log path for benchmark runs.
#[must_use]
pub fn default_log_path() -> PathBuf {
    std::env::temp_dir().join("rusty-skews-bench-live.log")
}

#[cfg(test)]
mod tests {
    use super::{parse_cpu_ticks, parse_status_rss};

    #[test]
    fn parses_cpu_ticks_with_spaces_in_the_process_name() {
        // Fields after the last ')': state, ppid, pgrp, session, tty, tpgid,
        // flags, minflt, cminflt, majflt, cmajflt, utime, stime, ...
        let stat = "12345 (rusty-skews (dev)) S 1 2 3 4 5 6 7 8 9 10 120 45 16 17 18 19";

        assert_eq!(parse_cpu_ticks(stat), Some(120 + 45));
    }

    #[test]
    fn rejects_malformed_stat_lines() {
        assert_eq!(parse_cpu_ticks("garbage"), None);
        assert_eq!(parse_cpu_ticks("1 (x) S 1 2"), None);
    }

    #[test]
    fn parses_rss_and_peak() {
        let status = "Name:\trusty-skews\nVmRSS:\t  102400 kB\nVmHWM:\t  204800 kB\n";

        assert_eq!(parse_status_rss(status), Some((102400, 204800)));
    }

    #[test]
    fn peak_falls_back_to_rss_when_missing() {
        let status = "VmRSS:\t  4096 kB\n";

        assert_eq!(parse_status_rss(status), Some((4096, 4096)));
    }

    #[test]
    fn rejects_status_without_rss() {
        assert_eq!(parse_status_rss("Name:\tfoo\n"), None);
    }
}
