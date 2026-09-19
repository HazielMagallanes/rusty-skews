# ADR-0002: Demand-driven frames with a serializable scene

* Status: accepted
* Date: 2026-09-19

## Context

Desktop shells spend almost all of their lifetime displaying a static UI.
Naive UI stacks redraw continuously (immediate mode) or allocate per frame,
which shows up as idle CPU and battery drain.

## Decision

1. **On-demand frames**: the kernel decides when to render; a static shell
   draws zero frames.
2. **Scene as data**: `skews-ui` produces a `Scene` (draw list + damage rects)
   that is serializable and snapshot-testable. `skews-render` consumes it.
3. **Frame callbacks** (`wl_surface.frame`) are requested only while
   animations run.
4. **Zero steady-state allocations**: draw lists are built in arenas and
   reused across frames.

## Consequences

* Idle CPU and per-frame allocations become measurable and testable.
* Damage tracking must be correct from the start; tests assert damage rects in
  scene snapshots.
* Immediate-mode convenience is given up; the element API is retained-style
  but rebuilt only on state change.
