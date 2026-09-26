# rusty-skews

> 🌐 **English** · [Español](docs/es/readme.md)

A skewed, Rust-native Wayland desktop shell for Hyprland (and other
layer-shell compositors), built for **performance, security and scalability**.

> Inspired by the UI/UX of [skwd](https://github.com/HazielMagallanes/skwd),
> but architected from scratch: layered hexagonal kernel, effects-as-data state
> model, config-driven module composition and a demand-driven GPU renderer.

**Status:** M1 — kernel, UI core, runtime and bar modules are in place; the bar
renders workspaces, clock, CPU temperature, memory and battery live.

---

## Architecture at a glance

```
L3  skews-app / skews-shell        kernel: update(msg) -> Effects, registry, bins
L2  skews-wayland, skews-render    driving adapters (layer surfaces, wgpu)
L2  skews-ui, skews-text, skews-layout
L1  skews-services, skews-ipc-hyprland, skews-sys-linux, skews-bus
L0  skews-core, skews-config, skews-theme   (no I/O, trivially testable)
```

* **Effects-as-data**: `update()` mutates state and returns commands; behavior
  is unit-testable without a display.
* **On-demand frames**: the renderer draws only when state changes.
* **Config-driven composition**: bar regions and modules live in TOML; adding a
  widget never touches the core.
* **`#![forbid(unsafe_code)]`** everywhere; the single audited `unsafe` block is
  wgpu surface creation in `skews-render`.

Read [docs/en/architecture.md](docs/en/architecture.md) for details, and
[docs/adr/](docs/adr) for the decisions behind it.

---

## Quick start

> New here? The [getting started guide](docs/en/getting-started.md) walks
> through prerequisites, the demo, configuration and troubleshooting.

Requirements: Rust 1.98+, a Wayland session (Hyprland recommended), Vulkan or
GL, and `libwayland` (the `system` backend is used to hand surfaces to wgpu).

```sh
# Diagnostics: checks Wayland, compositor, GPU, config and theme.
cargo run -p skews-ctl -- doctor

# Run the shell (renders a themed bar on every output).
cargo run -p skews-shell

# Full local CI gate (fmt, clippy, tests, docs, deny, audit).
cargo xt ci
```

Configuration defaults to `$XDG_CONFIG_HOME/rusty-skews/config.toml`; the
palette is read from `$XDG_CACHE_HOME/rusty-skews/colors.json` (matugen style,
flat `token -> "#rrggbb"`). Missing files fall back to built-in defaults.

```toml
[bar]
height = 32
monitor = "*"

[bar.left]
modules = ["workspaces", "media"]
[bar.center]
modules = ["clock"]
[bar.right]
modules = ["cpu", "memory", "battery", "volume"]
```

---

## Milestones

| Milestone | Scope | Status |
|---|---|---|
| M0 | Scaffold, CI, ADRs, doctor, hello bar on all outputs | ✅ |
| M1 | Kernel + UI core (text, layout, render, ui) + bar modules | ✅ |
| M2 | Panels, services (audio/network/bluetooth/media/notify/tray), OSD | ⏳ |
| M3 | Launcher with providers (apps, math, actions) | ⏳ |
| M4 | Wallpaper selector and theming pipeline | ⏳ |
| M5 | Window switcher and session lock (PAM) | ⏳ |
| M6 | Settings and packaging (v0.1) | ⏳ |

## Workspace layout

```
crates/
  skews-core      types, Msg/Effects, ports (no I/O)
  skews-config    TOML schema, validation, path resolution
  skews-theme     tokens from matugen JSON + defaults
  skews-ipc-hyprland  typed compositor events
  skews-sys-linux     /proc,/sys readers (coretemp + k10temp)
  skews-bus           D-Bus helpers (M1+)
  skews-services      service adapters (M1+)
  skews-text          shaping + glyph atlas (M1+)
  skews-layout        taffy integration (M1+)
  skews-render        wgpu renderer (SDF rects today, batcher in M1)
  skews-ui            elements/styles/animation/scene (M1+)
  skews-wayland       layer-shell glue
  skews-app           kernel + registry (M1)
  skews-modules       built-in modules (M1+)
  skews-shell         the `rusty-skews` binary
  skews-lock          the `rusty-skews-lock` binary (M5)
  skews-ctl           the `rusty-skews-ctl` binary (doctor today)
  skews-testkit       fakes, fixtures, scene assertions (M1+)
xtask/  docs/  tests/  benches/
```

## Documentation

| Document | English | Español |
|---|---|---|
| Getting started | [en](docs/en/getting-started.md) | [es](docs/es/getting-started.md) |
| Architecture | [en](docs/en/architecture.md) | [es](docs/es/architecture.md) |
| Performance budgets | [en](docs/en/performance.md) | [es](docs/es/performance.md) |
| Testing strategy | [en](docs/en/testing.md) | [es](docs/es/testing.md) |
| Threat model | [en](docs/en/threat-model.md) | [es](docs/es/threat-model.md) |
| Translation guide | [en](docs/en/translation-guide.md) | [es](docs/es/translation-guide.md) |
| Decisions (ADRs) | [en only](docs/adr) | — |

See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow and
[SECURITY.md](SECURITY.md) for vulnerability reporting.

## License

MIT — see [LICENSE](LICENSE).
