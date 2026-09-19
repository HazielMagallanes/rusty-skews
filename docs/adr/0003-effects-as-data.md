# ADR-0003: Effects-as-data kernel

* Status: accepted
* Date: 2026-09-19

## Context

Shell behavior mixes input handling, compositor protocol calls, service I/O
and rendering. Keeping that testable without a live session is the difference
between a stable project and a pile of manual testing. Prior art: Elm-style
`update(msg) -> Command`, Redux-style reducers, and the actor model.

## Decision

The kernel owns a single `State` on the main thread. All inputs become `Msg`s;
`update(&mut self, msg) -> Effects` mutates state synchronously and returns an
ordered list of side-effect descriptions (`Redraw`, `SendService`, `RunAction`,
`Persist`, `Notify`, …). The runtime executes effects; I/O happens in actors on
a dedicated tokio thread; cross-thread traffic uses bounded channels with
event coalescing.

## Consequences

* Behavior is unit-testable: feed messages, assert state and scene diffs.
* Effects are data, so they can be snapshot-tested and audited.
* Actors add boilerplate and channel plumbing; kept manageable by one actor per
  service and a small runtime.
* Debuggability improves: a message log reproduces issues deterministically.
