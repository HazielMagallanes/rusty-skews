# Testing

TDD is the working mode: every behavior change starts as a failing test.

## Test classes

| # | Class | Tools | What it covers |
|---|---|---|---|
| 1 | Unit + property | `cargo test`, `proptest` (M1+) | Pure logic: geometry, parsers, config validation, ranking, layout math |
| 2 | Snapshot | `insta` (M1+) | `Scene` draw lists, config serialization, app cache, search ranking |
| 3 | D-Bus integration | `dbus-run-session` + `skews-testkit` fakes | Services: battery, MPRIS, notifications, tray |
| 4 | Wayland integration | `labwc`/`sway` with `WLR_BACKENDS=headless` | Layer-surface lifecycle, output hotplug, panels |
| 5 | GPU goldens | wgpu on Mesa **lavapipe** (`vulkan-swrast`) | Offscreen render + SSIM image comparison |
| 6 | Performance | `criterion`, `dhat` | Hot-path benches and zero-allocation assertions |

Deterministic tests never require the developer's live session; the live
session is used only by `cargo xt demo`.

## Running

```sh
cargo xt test                 # nextest when installed, cargo test otherwise
cargo test -p skews-core      # a single crate
cargo xt demo                 # live smoke run for 5 seconds
```

Fixtures live next to the crate that owns the parsing logic
(`crates/<crate>/tests/fixtures/`) and are referenced through
`CARGO_MANIFEST_DIR`, so tests run from any directory.

## Policy

* No flaky tests in CI. A test that cannot be made deterministic is marked
  `#[ignore = "reason"]` and tracked.
* Integration tests that need a test-only daemon (D-Bus, headless compositor)
  are gated behind environment detection and fail with an actionable message.
* Snapshot changes are reviewed like code: an `insta` snapshot diff in a PR is
  a behavior change and must be justified.
