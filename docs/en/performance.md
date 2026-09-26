# Performance

> 🌐 **English** · [Español](../es/performance.md)

The shell targets desktop-feel performance: zero idle cost, bounded frame
latency and no allocation churn.

## Budgets

| Metric | Budget |
|---|---|
| Idle CPU (bar visible, no animation) | < 0.2 % of one core |
| Cold start → first bar frame | < 150 ms (target 80 ms) |
| RSS idle (bar only) | < 60 MB |
| RSS with 3k-app launcher cache | < 120 MB |
| Static bar frame CPU | < 1 ms |
| Animation frame CPU @ 120 Hz | < 4 ms |
| Launcher open (warm cache) | < 16 ms |
| App-cache rebuild (3k entries) | < 50 ms |
| Steady-state heap allocations per frame | 0 |

## Measured (M1 acceptance, 2026-09-25)

Development machine: Intel Iris Xe (Vulkan), Hyprland, 1920×1080 at scale 1,
release build, 60 s window plus a 15 min soak.

| Metric | Measured | Budget |
|---|---|---|
| Cold start → first frame | 81 ms (60 s run) / 91 ms (soak run) | < 150 ms |
| Idle CPU | 0.12 % (60 s) / 0.11 % (15 min soak) | < 0.2 % |
| RSS idle | 31.6 MiB (60 s) / 34.4 MiB after 15 min, no drift | < 60 MiB |

Findings from the acceptance run:

* The wgpu instance must request **Vulkan only**: `Backends::all()` also
  initializes the GL driver, which maps over 100 MiB of extra memory on Mesa.
  GL remains as a fallback when no Vulkan adapter exists.
* Swapchain reconfiguration is expensive; surfaces are only reconfigured when
  their size changes.
* Clocks and sensors tick every 2 s (`TICK_INTERVAL` in `skews-shell`), which
  keeps idle CPU well inside budget. Per-module intervals arrive in M2.
* Steady-state zero allocations (ADR-0002) is **not yet enforced**: the scene
  builder still allocates per redraw. Arena reuse is planned for the UI-core
  hardening slice.

## How budgets are enforced

* **Design**: demand-driven frames, damage tracking, glyph/quads reuse, no
  polling loops except sensors (≥ 1 s intervals), event coalescing.
* **Tests**: `dhat`-based zero-allocation assertions on the steady path;
  criterion benches for hot pure logic (config parse, ranking, layout);
  `Scene` build benches.
* **Local measurement**: `cargo xt bench-live` (M1) reports RSS and frame
  timings from `/proc` and wgpu timestamps. Numbers from CI machines are
  indicative only; acceptance runs are done on-device.

## Build profiles

| Profile | Purpose | Key settings |
|---|---|---|
| `dev` | iteration | default |
| `release` | shipping | `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip`, `opt-level=3` |
| `release-debug` | profiling | release + debuginfo |
| `bench` | criterion | inherits release (unwinding) |

Local builds may add `-C target-cpu=native`; release artifacts for other
machines must not.

## Methodology notes

* Measure with the compositor running a realistic scene (bar + one panel open),
  at least 60 s after startup so caches are warm.
* Report: peak and steady RSS, idle CPU over 5 minutes, frame CPU p50/p95 while
  animating, first-frame latency from process start.
* Regressions beyond 20 % on any budget require an explanatory ADR note in the
  PR.
