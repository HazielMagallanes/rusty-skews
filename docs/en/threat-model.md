# Threat model

> 🌐 **English** · [Español](../es/threat-model.md)

## Assets

1. The user's session (input focus, screen contents, lock state).
2. Command execution (launching apps, compositor actions).
3. Credentials in transit (PAM in M5, any future secrets).
4. The build supply chain.

## Adversaries and surfaces

| Surface | Threat | Mitigation |
|---|---|---|
| `.desktop` files (untrusted, user-writable) | Code execution via `Exec` | Parse `Exec` and `execvp` directly; never `sh -c`; fuzz the parser (ADR-0005) |
| Icon themes / SVG | Parsing exploits | `resvg`/`usvg` with size limits (M1+); fuzz |
| Hyprland IPC stream | Panic/DoS via malformed events | Panic-free typed parser; length caps |
| D-Bus services (MPRIS, notifications, tray) | Spoofed payloads, oversized images | Validate/size-limit payloads; zbus types; fuzz notification payloads |
| Config / palette files | Local code execution, path traversal | Pure data parsing; no shell; paths resolved without `..` escapes |
| Network (weather, ollama) | Content injection | TLS via rustls; renderer treats responses as data |
| Lockscreen (M5) | Lock bypass | Dedicated minimal binary, no IPC surface while locked, `ext-session-lock-v1`, PAM in an isolated FFI crate, threat-model document, fuzz + test matrix |
| Supply chain | Malicious dependency | `cargo-deny` (advisories, licenses, bans, sources), `cargo-audit`, pinned lockfile, `--locked` CI, Renovate |

Known tracked exception: `ttf-parser` (RUSTSEC-2026-0192) is unmaintained but
not vulnerable; it is transitive through `cosmic-text → fontdb` and is ignored
in `deny.toml` until those crates migrate to `skrifa`.

## Invariants

* `unsafe` is forbidden workspace-wide. The single exception is the audited
  wgpu raw-handle bridge in `skews-render`; PAM FFI will be the second,
  isolated in `skews-lock`.
* Untrusted input never reaches a shell, `eval`, or a format string.
* Secrets (none today; PAM in M5) are never logged; `Debug` impls on secret
  types are hand-written to redact.
* Panics are contained at actor boundaries; the kernel converts them into
  visible failures instead of silent corruption.
