# Changelog

> 🌐 **English** · [Español](docs/es/changelog.md)

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) once it
ships releases.

> New entries are written in English first; the Spanish changelog is updated on
> the next translation pass (see the
> [translation guide](docs/en/translation-guide.md)).

## [Unreleased]

### Added

* M1 acceptance tooling: `cargo xt bench-live` builds the release shell, waits
  for the first frame and reports cold start, idle CPU and RSS against the
  documented budgets (`--seconds` for soak runs).
* Performance fixes from the acceptance run: the wgpu instance requests
  **Vulkan only** (GL stays as a fallback), surfaces are reconfigured only when
  their size changes, and clocks/sensors tick every 2 s. Measured on device:
  81 ms cold start, 0.12 % idle CPU, 31.6 MiB RSS — all budgets pass.
* Docs: the performance budgets now record the M1 acceptance measurements.
* Runtime: calloop event loop with a two-second tick timer (clock and sensors
  update live), configuration hot reload with identical-write deduplication and
  last-good fallback, and automatic surface reconfiguration when the bar height
  changes.
* Compositor integration: Hyprland command-socket queries and the `.socket2`
  event stream drive the `workspaces` module.
* Modules: `workspaces` (Hyprland IPC) and `battery` (power-supply reader with
  a charging suffix) replace their placeholders.
* Config schema now matches the documented form
  (`[bar.left] modules = [...]`).
* UI core: `skews-text` (cosmic-text shaping with system fallback, shape cache,
  glyph rasterization), `skews-layout` (taffy flex layout for the bar regions),
  `skews-ui` (serializable scenes) and a wgpu glyph atlas + text pipeline; the
  bar now renders module outputs (clock, CPU temperature, memory).
* Supply chain: the `ttf-parser` unmaintained advisory (RUSTSEC-2026-0192) is
  documented as a tracked exception in `deny.toml` and the threat model.
* M1 kernel: effects-as-data `Shell` (`update(msg) -> Effects`) with the
  compile-time module registry (ADR-0004); unknown module ids are rejected
  with the list of known modules, and invalid reloads keep the last good
  configuration.
* Modules: `clock` (configurable strftime format), `cpu` (Intel `coretemp` +
  AMD `k10temp`, fixture-tested) and `memory`; `workspaces` and `battery` are
  registered placeholders until their slices land.
* The shell binary now composes the bar plan from the kernel (config-driven
  regions and module outputs) and executes kernel effects.
* M0 scaffold: 19-crate workspace with strict layer boundaries and
  `forbid(unsafe_code)`; `rusty-skews`, `rusty-skews-ctl` and
  `rusty-skews-lock` binaries.
* `skews-core`: geometry with damage-friendly intersection/union math, RGBA hex
  parsing, `Msg`/`Effect` vocabulary (effects-as-data kernel types).
* `skews-config`: versioned TOML schema with validation (bar height, duplicate
  modules, empty ids), defaults and XDG path resolution.
* `skews-theme`: matugen-style palette ingestion with safe fallback palette.
* `skews-ipc-hyprland`: typed, panic-free parsing of Hyprland IPC events.
* `skews-sys-linux`: hwmon temperature reader supporting both **Intel
  (coretemp)** and AMD (k10temp), memory and CPU-usage readers with fixtures.
* `skews-wayland` + `skews-render`: layer-shell bar surfaces on every output
  with a wgpu SDF rounded-rect pipeline (demand-driven; one frame per configure).
* `rusty-skews-ctl doctor`: environment checks (Wayland, compositor, GPU,
  config, theme) with `--strict`.
* Docs: architecture, performance budgets, testing strategy, threat model,
  ADRs 0001–0005; CI workflow (fmt, clippy, tests, docs, deny, audit).
* Documentation is split into `docs/en/` and `docs/es/` with per-file language
  selectors and a translation guide for contributors.
* Onboarding: getting started guide (EN/ES) covering prerequisites, the demo,
  configuration and troubleshooting.
* Community files for the published repository: Code of Conduct
  ([Contributor Covenant 2.1](CODE_OF_CONDUCT.md), EN/ES), issue templates
  (bug report, feature request), a pull request template and Dependabot
  configuration.
* Repository published at
  [github.com/HazielMagallanes/rusty-skews](https://github.com/HazielMagallanes/rusty-skews):
  public, CI-gated `main` (fmt, clippy, tests, docs, cargo-deny, cargo-audit),
  Dependabot updates and security alerts enabled.
