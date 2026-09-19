# ADR-0005: Never interpolate into a shell

> 🌐 **English** · *ADRs are maintained in English only / los ADR se mantienen únicamente en inglés*

* Status: accepted
* Date: 2026-09-19

## Context

The predecessor shell builds command lines as strings and runs them through
`bash -c` (for `jq` mutations, `hyprctl`, `wpctl`, `wget`, …). That pattern is
a persistent source of quoting bugs and a code-execution surface: any app name,
title, path or notification payload containing quotes, backslashes or `$()`
changes the meaning of the command.

## Decision

1. rusty-skews **never** passes interpolated data to a shell.
2. External programs are spawned with `std::process::Command` and argument
   arrays (no `sh -c`).
3. `.desktop` `Exec` values are parsed into argv and executed with `execvp`
   semantics, honoring `Terminal=true` by exec'ing the configured terminal with
   the parsed argv — never by concatenating a string.
4. Where a protocol exists (Hyprland IPC, PipeWire, BlueZ, D-Bus), talk the
   protocol instead of shelling out to CLIs.
5. Any future exception (a tool that exists only as a CLI) must be documented
   in the module, pass arguments as arrays, and fuzz its inputs.

## Consequences

* Quoting bugs and shell injection disappear as a class.
* Some integrations become more work (protocol clients instead of `wpctl`), but
  they are also faster and observable.
* Tests for parsers and exec plans are pure and fuzzable.
