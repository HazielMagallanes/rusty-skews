# ADR-0001: Rendering stack — sctk + wgpu + cosmic-text + taffy

* Status: accepted
* Date: 2026-09-19

## Context

rusty-skews must be maximally performant, secure and testable while remaining
maintainable by a small team. The alternatives considered were:

1. `iced` + `iced_layershell` (proven for bars, Elm architecture, wgpu).
2. `slint` + sctk glue (small footprint, declarative markup).
3. `egui` (fast to prototype, immediate-mode repaints).
4. Custom stack: `smithay-client-toolkit` + `wgpu` + `cosmic-text` + `taffy`.

## Decision

Build the shell directly on `smithay-client-toolkit` for Wayland plumbing,
`wgpu` for rendering, `cosmic-text` for shaping/fallback, and `taffy` for
flex/grid layout. The UI layer (elements, styles, animations, scene) is ours,
kept small and snapshot-testable.

## Consequences

* Maximum control over frame scheduling, damage tracking and allocations —
  required to meet the budgets in `docs/performance.md`.
* We own a widget layer, text integration and animation system; scope is
  mitigated by leaning on `taffy` (layout) and `cosmic-text` (text) instead of
  writing those engines, and by freezing the frame/scene model in ADR-0002.
* `wgpu` raw-handle surface creation requires the Wayland **system** backend
  (`client_system`), so `libwayland-client` is a runtime dependency by design.
* UI logic stays display-free testable because scenes are plain data.

## Revisit when

`cosmic-text` or `taffy` stop fitting (e.g. text quality at fractional scales,
or layout expressiveness), or when the widget layer costs more than adopting
`iced`/`slint` wholesale.
