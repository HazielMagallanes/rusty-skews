# Contributing

> 🌐 **English** · [Español](docs/es/contributing.md)

Thanks for helping build rusty-skews. This project is developed with a strict
TDD workflow and a reviewable safety/performance budget.

## Ground rules

1. **Tests first.** Every behavior change starts as a failing test in the crate
   that owns the logic (see [docs/en/testing.md](docs/en/testing.md)).
2. **Layers point downward.** `skews-core` never depends on adapters; adapters
   never depend on the kernel. Adding a dependency edge that violates
   [docs/en/architecture.md](docs/en/architecture.md) requires an ADR.
3. **No `unsafe`.** The workspace lints forbid it. The single exception is the
   audited wgpu surface bridge in `skews-render`; PAM FFI will be similarly
   isolated in `skews-lock`.
4. **No shell interpolation.** Never pass user data through `sh -c`; parse and
   `execvp`, or talk protocols directly (see
   [ADR-0005](docs/adr/0005-no-shell-interpolation.md)).
5. **Conventional Commits** (`feat:`, `fix:`, `perf:`, `docs:`, `chore:`,
   `security:` …). Keep commits focused.
6. **Docs move with code.** Update README/docs tables and ADRs in the same PR.

## Documentation and translations

Documentation is **one language per file**, organized in language folders:

* English guides live in `docs/en/`; Spanish translations in `docs/es/`.
* Root files (`README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`)
  are the English canonical versions; their Spanish counterparts live in
  `docs/es/` (`readme.md`, `contributing.md`, `security.md`, `changelog.md`).
* Every document starts with a language selector line linking to its
  counterpart.
* ADRs (`docs/adr/`) are maintained in English only.

When you add or change a document, follow the
[translation guide](docs/en/translation-guide.md): update the English source
and either update the Spanish file in the same PR or mark it as outdated.

## Local gate

> New here? Start with the
> [getting started guide](docs/en/getting-started.md).

```sh
cargo xt ci          # fmt + clippy + tests + doc + deny + audit
cargo xt fmt         # formatting only
cargo xt test        # nextest when installed, cargo test otherwise
cargo xt demo        # runs the shell for 5s against the live session
```

## Adding a module (from M1)

Modules are the unit of extension. The recipe:

1. Create the widget/service in `skews-modules` (split to its own crate when it
   grows).
2. Implement `Module` (id, init, update, optional bar_view/panels/launcher).
3. Register it in `skews-shell`'s compile-time registry.
4. Add its defaults to the config schema if it has options.
5. Ship tests: pure logic unit tests, a config validation test, and — when it
   renders — a `Scene` snapshot.

The core loop must not change for new modules; if it does, that is an ADR.

## Review checklist

* [ ] Tests cover the new behavior and the old behavior stays covered.
* [ ] `cargo xt ci` passes locally.
* [ ] Public items documented; docs/ADRs updated.
* [ ] New/changed docs have a selector and a translated counterpart
      (or are marked outdated).
* [ ] No new `unsafe`; no new dependence cycles.
* [ ] Perf-sensitive code has a criterion bench or a documented reason.
