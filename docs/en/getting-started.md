# Getting started

> 🌐 **English** · [Español](../es/getting-started.md)

Everything you need to go from a fresh clone to seeing rusty-skews running on
your own session, in about ten minutes.

## 1. Prerequisites

| Requirement | Notes |
|---|---|
| Rust | The pinned toolchain (1.98.1) is in `rust-toolchain.toml`; rustup installs it automatically on first build. |
| Wayland session | Hyprland is the primary target. Any wlroots/smithay compositor with `wlr-layer-shell` works; GNOME does not support it. |
| GPU with Vulkan | Mesa or vendor drivers. GL works as a fallback; `rusty-skews-ctl doctor` tells you what was found. |
| Build dependencies | `libwayland`, `pkg-config` and a C toolchain. |

Arch example:

```sh
sudo pacman -S --needed base-devel pkgconf libwayland
```

## 2. Build and check

```sh
cd ~/codigos/personal/rust/rusty-skews

# Full local gate: fmt, clippy, tests, docs, deny, audit.
cargo xt ci

# Environment check: Wayland, compositor, GPU, config, theme.
cargo run -p skews-ctl -- doctor

# Live budgets: cold start, idle CPU and RSS of the release build.
cargo xt bench-live
```

`doctor` exits non-zero when something required is missing; add `--strict` to
treat warnings (missing config/theme, unknown compositor) as failures.

## 3. Run the demo

The demo runs the real shell against your live session for a few seconds and
then exits by itself:

```sh
cargo xt demo              # 5 seconds
cargo xt demo --seconds 10 # longer look
```

What you should see:

* A 32 px bar anchored to the top of **every output**.
* Bar background from the theme's `surface_container` color at 90% alpha, with
  a small `primary`-colored dot in the center (placeholder for M0).
* While it runs, the bar reserves an exclusive zone, so tiling layouts make
  room for it; when it exits, the zone is released.

Equivalent manual run (the demo just wraps this):

```sh
cargo run -p skews-shell -- --exit-after 6
```

Useful flags for `skews-shell`: `-c/--config <path>`, `-v` (repeatable for more
logs), `--exit-after <seconds>` (development helper).

> The M0 bar renders only; there are no interactive modules yet (clock,
> workspaces, sensors are M1).

## 4. Try it as your shell (optional)

This is still an M0 scaffold, so treat it as a test drive. If you use another
shell (for example the skwd setup), remember both bars would reserve space at
the same time.

```sh
# Stop the currently running Wayland shell (example: quickshell-based skwd).
pkill -f 'quickshell -p /usr/share/skwd'

# Run rusty-skews in the foreground; Ctrl+C to stop.
cargo run -p skews-shell

# Restore the previous shell (example).
setsid env QT_QUICK_BACKEND=software skwd >/dev/null 2>&1 &
```

## 5. Configuration

Both files are optional; missing files fall back to built-in defaults and
`doctor` reports a warning.

* Config: `$XDG_CONFIG_HOME/rusty-skews/config.toml` (usually
  `~/.config/rusty-skews/config.toml`).

  ```toml
  [bar]
  height = 32
  monitor = "*"          # or "eDP-1", "HDMI-A-1", ...

  [bar.left]
  modules = ["workspaces"]
  [bar.center]
  modules = ["clock"]
  [bar.right]
  modules = ["cpu", "memory", "battery", "network", "audio", "tray"]
  ```

  Modules referenced here are validated against the compile-time registry at
  startup; unknown ids are rejected with the list of known modules.

* Palette: `$XDG_CACHE_HOME/rusty-skews/colors.json` (usually
  `~/.cache/rusty-skews/colors.json`), a flat map in matugen style:

  ```json
  { "primary": "#89b4fa", "surface_container": "#313244", "on_surface": "#cdd6f4" }
  ```

  Unknown keys are ignored; invalid colors are reported by name.

## 6. Troubleshooting

| Symptom | Fix |
|---|---|
| `doctor` fails on `wayland` | Run from inside the graphical session, not over SSH; check `WAYLAND_DISPLAY`. |
| `doctor` fails on `gpu` | Install a Vulkan driver (`mesa`, vendor ICDs) or try `WGPU_BACKEND=gl`. |
| Build error about `wayland-sys`/`libwayland` | Install `libwayland` and `pkg-config`. |
| Shell exits with "failed to bind bar surfaces" | Compositor lacks `wlr-layer-shell` (e.g. GNOME); use Hyprland or another wlroots/smithay compositor. |
| Bar not visible | The demo exits after N seconds; try `--seconds 15` or run `cargo run -p skews-shell` with `-vv` and watch the logs. |

## 7. Where to go next

* [Architecture](architecture.md) — how the kernel, modules and renderer fit.
* [Testing](testing.md) — the six test classes and how to run them.
* [Performance](performance.md) — budgets and how they are measured.
* [Contributing](../../CONTRIBUTING.md) — workflow and how to add a module (M1+).
* [Milestones](../../README.md#milestones) — what is being built next.
