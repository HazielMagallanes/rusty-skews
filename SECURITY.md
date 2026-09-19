# Security Policy

## Reporting a vulnerability

Report privately via GitHub Security Advisories on
`HazielMagallanes/rusty-skews`, or by email to the address in the git history.
Please include a reproduction and the affected milestone/crate. We aim to
acknowledge within 72 hours.

Do **not** open public issues for exploitable problems.

## Scope

rusty-skews is a Wayland client that runs as your user and talks to
compositor, D-Bus services and the network on your behalf. In-scope issues
include, for example:

* Parsing of untrusted input: `.desktop` files, icon themes, Hyprland IPC
  events, notification payloads, config and theme files (all fuzz targets).
* Command execution: launching apps must never go through a shell; a
  `.desktop` `Exec` that can escape into shell interpretation is a vulnerability
  (see ADR-0005).
* Session lock (`skews-lock`, M5): bypassing the lock surface, IPC exposure
  while locked, PAM misuse, or lock-state desynchronization.
* Supply chain: unexpected network/filesystem behavior from dependencies.

## Baseline guarantees

* `#![forbid(unsafe_code)]` across the workspace; the only exception is the
  audited wgpu raw-handle bridge in `skews-render`.
* No secrets are logged; palette/config parsing never executes content.
* `cargo-deny` + `cargo-audit` run in CI; dependency additions are reviewed for
  license, maintenance and unsafe usage.

## Threat model

See [docs/security.md](docs/security.md) for the full model, including the
lockscreen threat analysis landing in M5.
