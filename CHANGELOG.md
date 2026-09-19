# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) once it
ships releases.

## [Unreleased]

### Added

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
* Docs: architecture, performance budgets, testing strategy, security notes,
  ADRs 0001–0005; CI workflow (fmt, clippy, tests, docs, deny, audit).
