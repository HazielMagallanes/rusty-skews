//! Environment checks for running rusty-skews.

use skews_config::{Config, default_config_path};
use skews_theme::default_palette_path;

/// Result of a single environment check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Everything is in place.
    Ok,
    /// The shell can run, but something is missing or degraded.
    Warn,
    /// The shell cannot run in this environment.
    Fail,
}

/// One named check with its outcome.
#[derive(Debug, Clone)]
pub struct Check {
    /// Short check name.
    pub name: &'static str,
    /// Outcome status.
    pub status: Status,
    /// Human-readable detail.
    pub detail: String,
}

/// Aggregated doctor report.
#[derive(Debug, Clone)]
pub struct Report {
    /// Rendered report text.
    pub text: String,
    /// Process exit code (0 = healthy).
    pub exit_code: i32,
}

/// Runs every check and renders a report.
pub fn run(strict: bool) -> Report {
    let checks = vec![
        check_wayland(),
        check_compositor(),
        check_gpu(),
        check_config(),
        check_theme(),
    ];
    render(&checks, strict)
}

/// Renders checks into a report and computes the exit code.
#[must_use]
pub fn render(checks: &[Check], strict: bool) -> Report {
    let mut text = String::from("rusty-skews doctor\n");
    for check in checks {
        text.push_str(&format!(
            "  [{}] {:<12} {}\n",
            glyph(check.status),
            check.name,
            check.detail
        ));
    }

    let failures = checks
        .iter()
        .filter(|check| check.status == Status::Fail)
        .count();
    let warnings = checks
        .iter()
        .filter(|check| check.status == Status::Warn)
        .count();
    let exit_code = if failures > 0 || (strict && warnings > 0) {
        1
    } else {
        0
    };

    if failures > 0 {
        text.push_str(&format!("\n{failures} check(s) failed\n"));
    } else if warnings > 0 {
        text.push_str(&format!("\n{warnings} warning(s)\n"));
    } else {
        text.push_str("\nall checks passed\n");
    }

    Report { text, exit_code }
}

const fn glyph(status: Status) -> char {
    match status {
        Status::Ok => 'o',
        Status::Warn => '~',
        Status::Fail => 'x',
    }
}

fn check_wayland() -> Check {
    match skews_wayland::Connection::connect_to_env() {
        Ok(_) => Check {
            name: "wayland",
            status: Status::Ok,
            detail: String::from("connected to the compositor"),
        },
        Err(error) => Check {
            name: "wayland",
            status: Status::Fail,
            detail: format!("cannot connect: {error}"),
        },
    }
}

fn check_compositor() -> Check {
    let compositor = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .ok()
        .map(|_| "hyprland")
        .map(String::from);

    match compositor {
        Some(name) => Check {
            name: "compositor",
            status: Status::Ok,
            detail: format!("{name} detected"),
        },
        None => Check {
            name: "compositor",
            status: Status::Warn,
            detail: String::from(
                "no supported compositor signature found (rusty-skews targets Hyprland first)",
            ),
        },
    }
}

fn check_gpu() -> Check {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));

    match adapters.first() {
        Some(adapter) => {
            let info = adapter.get_info();
            Check {
                name: "gpu",
                status: Status::Ok,
                detail: format!("{} ({:?})", info.name, info.backend),
            }
        }
        None => Check {
            name: "gpu",
            status: Status::Fail,
            detail: String::from("no wgpu adapter available"),
        },
    }
}

fn check_config() -> Check {
    let path = default_config_path();
    match Config::load(&path) {
        Ok(_) => Check {
            name: "config",
            status: Status::Ok,
            detail: format!("{} is valid", path.display()),
        },
        Err(skews_config::ConfigError::Io { .. }) => Check {
            name: "config",
            status: Status::Warn,
            detail: format!("{} not found; defaults will be used", path.display()),
        },
        Err(error) => Check {
            name: "config",
            status: Status::Fail,
            detail: format!("{}: {error}", path.display()),
        },
    }
}

fn check_theme() -> Check {
    let path = default_palette_path();
    if path.is_file() {
        Check {
            name: "theme",
            status: Status::Ok,
            detail: format!("{} present", path.display()),
        }
    } else {
        Check {
            name: "theme",
            status: Status::Warn,
            detail: format!("{} not found; using built-in palette", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Check, Status, glyph, render};

    #[test]
    fn render_reports_success_when_all_checks_pass() {
        let checks = vec![Check {
            name: "a",
            status: Status::Ok,
            detail: String::new(),
        }];
        let report = render(&checks, false);

        assert_eq!(report.exit_code, 0);
        assert!(report.text.contains("all checks passed"));
    }

    #[test]
    fn render_fails_on_failures_regardless_of_strict() {
        let checks = vec![Check {
            name: "a",
            status: Status::Fail,
            detail: String::from("boom"),
        }];

        assert_eq!(render(&checks, false).exit_code, 1);
        assert_eq!(render(&checks, true).exit_code, 1);
        assert!(render(&checks, false).text.contains("boom"));
    }

    #[test]
    fn strict_mode_promotes_warnings_to_failure() {
        let checks = vec![Check {
            name: "a",
            status: Status::Warn,
            detail: String::new(),
        }];

        assert_eq!(render(&checks, false).exit_code, 0);
        assert_eq!(render(&checks, true).exit_code, 1);
    }

    #[test]
    fn glyphs_are_stable() {
        assert_eq!(glyph(Status::Ok), 'o');
        assert_eq!(glyph(Status::Warn), '~');
        assert_eq!(glyph(Status::Fail), 'x');
    }
}
