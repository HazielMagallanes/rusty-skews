# Architecture

> 🌐 **English** · [Español](../es/architecture.md)

> Status: M0. This document describes the target architecture; milestones land
> the pieces in order. ADRs record the decisions.

## 1. Layers (hexagonal)

Dependencies point **downward only**:

```
L3  skews-app            kernel: Shell::update(msg) -> Effects, registry, scheduling
L3  skews-shell          binary: config -> registry -> kernel
L3  skews-lock, skews-ctl binaries
L2  skews-wayland        driving adapter: layer surfaces, outputs, seat, frame callbacks
L2  skews-render         driving adapter: wgpu scene renderer
L2  skews-ui             elements, styles, animation, scene construction
L2  skews-text           shaping, font fallback, glyph atlas
L2  skews-layout         taffy integration + text measurement
L1  skews-ipc-hyprland   driven adapter: compositor events/actions
L1  skews-sys-linux      driven adapter: /proc,/sys sensors
L1  skews-bus            zbus helpers/runtime bridging
L1  skews-services.*     driven adapters: battery, audio, network, bluetooth, media, notify, tray, weather
L0  skews-core           ids, geometry, color, time, Msg/Effects, ports (no I/O)
L0  skews-config         schema, validation, migrations
L0  skews-theme          tokens
L4  skews-testkit        fakes, fixtures, scene assertions (test-only)
```

L0 crates have no I/O, no Wayland, no wgpu, no async runtime. That is what
makes the shell's behavior testable without a display.

## 2. Kernel: effects-as-data

```rust
fn update(&mut self, msg: Msg) -> Effects;
// Effect::Redraw | SetExclusiveZone(..) | SendService(..)
//        | RunAction(ActionId) | Persist(..) | Notify(..)
```

* Single writer: the main thread owns `State`; every input (Wayland, service,
  timer) becomes a `Msg`.
* `update` is synchronous and allocation-conscious; tests feed message streams
  and assert state + resulting `Scene` diffs.
* I/O lives in actors on a dedicated tokio thread, communicating over bounded
  channels; events coalesce to the latest value.
* Actor panics are caught at the actor boundary; the kernel stays alive and
  reports the failure. (Supervision policy lands with `skews-app`, M1.)

## 3. Frame model

* Frames are built only when dirty. A static bar draws zero frames.
* The UI layer emits a serializable `Scene` (draw list + damage rects). Tests
  snapshot scenes; the renderer consumes them.
* `wl_surface.frame` callbacks are requested only while animations run.
* Steady-state zero allocations are asserted with the `dhat` allocator.

## 4. Modules and config

Modules implement the `Module` trait and live in a compile-time registry in
`skews-shell` (feature-gated). Config references module ids; validation errors
point at the offending key. Panels are declared by modules and rendered as
dedicated layer surfaces with centralized grab/dismiss policy. Launcher
providers (apps, math, actions) plug into the launcher without touching it.

## 5. Threads

```
main (calloop)        Wayland, input, State::update, scene build, wgpu
tokio runtime thread  zbus clients/servers, IPC, HTTP, fs watchers
rayon pool            icon/thumbnail decode, color extraction
```

No locks in hot paths; bounded channels with coalescing between threads.

## 6. External helpers

`awww` (wallpaper daemon), `mpvpaper` (video wallpapers), `matugen` (color
generation) and `cava` (audio visualizer) stay external and are driven through
adapters. They are already optimized for their jobs; the shell owns
composition, not reimplementation.
