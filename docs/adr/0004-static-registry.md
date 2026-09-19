# ADR-0004: Static module registry, no dynamic plugins

> 🌐 **English** · *ADRs are maintained in English only / los ADR se mantienen únicamente en inglés*

* Status: accepted
* Date: 2026-09-19

## Context

The shell must scale with new widgets, panels and launcher providers without
touching the core. Dynamic loading (`cdylib`/`dlopen`) is tempting but brings
ABI instability, version skew, unsafe loading paths and packaging pain.

## Decision

Modules are **compile-time** participants: each implements the `Module` trait
and is registered in `skews-shell` behind Cargo features. Configuration
references module ids; unknown ids fail validation with suggestions. Hot paths
never consult a registry at runtime outside startup and config reload.

## Consequences

* No ABI surface; Rust's type system and release tooling apply to modules.
* Third parties extend by adding a crate to the workspace and a registry line —
  cheap for contributors, no runtime cost for users.
* Packagers choose feature sets (e.g. exclude the launcher) at build time.
* If out-of-tree binary plugins ever become a hard requirement, that warrants a
  new ADR with a process-isolation design rather than `dlopen`.
