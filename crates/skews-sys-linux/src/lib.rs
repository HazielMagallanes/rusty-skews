//! Linux `/proc` and `/sys` readers.
//!
//! Every reader takes the filesystem root as a parameter so tests can point it
//! at fixtures; production callers use [`SysPaths::default`].

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// Filesystem roots for the readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SysPaths {
    /// Directory containing `hwmon*` entries (typically `/sys/class/hwmon`).
    pub hwmon: PathBuf,
    /// `meminfo` file (typically `/proc/meminfo`).
    pub meminfo: PathBuf,
    /// `stat` file (typically `/proc/stat`).
    pub stat: PathBuf,
}

impl Default for SysPaths {
    fn default() -> Self {
        Self {
            hwmon: PathBuf::from("/sys/class/hwmon"),
            meminfo: PathBuf::from("/proc/meminfo"),
            stat: PathBuf::from("/proc/stat"),
        }
    }
}

/// Reads the CPU package temperature in Celsius from `hwmon`.
///
/// Supports Intel (`coretemp`) and AMD (`k10temp`) drivers. Returns `None`
/// when no supported sensor is present or its value cannot be parsed.
#[must_use]
pub fn cpu_temp_celsius(hwmon_root: &Path) -> Option<f32> {
    let entries = fs::read_dir(hwmon_root).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match fs::read_to_string(path.join("name")) {
            Ok(name) => name.trim().to_owned(),
            Err(_) => continue,
        };

        if name != "coretemp" && name != "k10temp" {
            continue;
        }

        let raw = match fs::read_to_string(path.join("temp1_input")) {
            Ok(raw) => raw,
            Err(_) => continue,
        };

        if let Ok(millidegrees) = raw.trim().parse::<i64>() {
            return Some(millidegrees as f32 / 1000.0);
        }
    }

    None
}

/// Reads the used memory percentage from `/proc/meminfo`.
///
/// Returns `None` when the required fields are missing or malformed.
#[must_use]
pub fn memory_used_percent(meminfo: &Path) -> Option<f32> {
    let text = fs::read_to_string(meminfo).ok()?;

    let mut total_kib = None;
    let mut available_kib = None;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kib = parse_kib(rest);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available_kib = parse_kib(rest);
        }
    }

    let (total, available) = (total_kib?, available_kib?);
    if total <= 0.0 {
        return None;
    }

    let used = (total - available).max(0.0);
    Some((used / total * 100.0) as f32)
}

fn parse_kib(value: &str) -> Option<f64> {
    value
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<f64>()
        .ok()
}

/// Stateful CPU usage sampler based on `/proc/stat` deltas.
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous: Option<(u64, u64)>,
}

impl CpuSampler {
    /// Creates an empty sampler; the first call establishes the baseline.
    #[must_use]
    pub const fn new() -> Self {
        Self { previous: None }
    }

    /// Samples the aggregate CPU usage percentage since the previous call.
    ///
    /// Returns `None` on the first call, when the file is missing, or when the
    /// counters did not advance.
    pub fn sample_percent(&mut self, stat: &Path) -> Option<f32> {
        let text = fs::read_to_string(stat).ok()?;
        let first_line = text.lines().next()?;
        let (total, idle) = parse_stat_line(first_line)?;

        let previous = self.previous.replace((total, idle));

        let (prev_total, prev_idle) = previous?;
        let delta_total = total.checked_sub(prev_total)?;
        let delta_idle = idle.checked_sub(prev_idle)?;
        if delta_total == 0 {
            return None;
        }

        let busy = delta_total.saturating_sub(delta_idle);
        Some((busy as f64 / delta_total as f64 * 100.0) as f32)
    }
}

fn parse_stat_line(line: &str) -> Option<(u64, u64)> {
    let mut fields = line.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }

    let mut values = [0_u64; 8];
    for (index, value) in fields.take(8).enumerate() {
        values[index] = value.parse::<u64>().ok()?;
    }

    let idle = values[3];
    let iowait = values[4];
    let total: u64 = values.iter().sum();

    Some((total, idle + iowait))
}

#[cfg(test)]
mod tests {
    use super::{CpuSampler, cpu_temp_celsius, memory_used_percent, parse_stat_line};
    use std::path::Path;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn reads_intel_coretemp() {
        let temp = cpu_temp_celsius(&fixture("hwmon-intel"));

        assert_eq!(temp, Some(52.0));
    }

    #[test]
    fn reads_amd_k10temp_after_unrelated_sensors() {
        let temp = cpu_temp_celsius(&fixture("hwmon-amd"));

        assert_eq!(temp, Some(47.5));
    }

    #[test]
    fn missing_sensor_returns_none() {
        assert_eq!(cpu_temp_celsius(&fixture("hwmon-none")), None);
    }

    #[test]
    fn reads_memory_percentage() {
        let percent = memory_used_percent(&fixture("meminfo")).unwrap();

        // 16 GiB total, 4 GiB available -> 75% used.
        assert!((percent - 75.0).abs() < 0.01);
    }

    #[test]
    fn malformed_meminfo_returns_none() {
        assert_eq!(memory_used_percent(&fixture("meminfo-broken")), None);
    }

    #[test]
    fn parses_proc_stat_line() {
        let line = "cpu  100 0 100 700 100 0 0 0 0 0";
        let (total, idle) = parse_stat_line(line).unwrap();

        assert_eq!(total, 1000);
        assert_eq!(idle, 800);
    }

    #[test]
    fn cpu_sampler_computes_delta() {
        let mut sampler = CpuSampler::new();

        // First sample establishes the baseline.
        assert_eq!(sampler.sample_percent(&fixture("proc-stat-a")), None);

        // Second sample: total delta 200, idle delta 100 -> 50% busy.
        let percent = sampler.sample_percent(&fixture("proc-stat-b")).unwrap();
        assert!((percent - 50.0).abs() < 0.01);

        // Re-sampling an unchanged file yields no counter progress.
        assert_eq!(sampler.sample_percent(&fixture("proc-stat-b")), None);
    }

    #[test]
    fn missing_stat_file_returns_none() {
        let mut sampler = CpuSampler::new();
        assert_eq!(
            sampler.sample_percent(Path::new("/nonexistent/proc/stat")),
            None
        );
    }
}
