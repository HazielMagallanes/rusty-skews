# Modelo de amenazas

> 🌐 [English](../en/threat-model.md) · **Español**

## Activos

1. La sesión del usuario (foco de entrada, contenido de pantalla, estado de
   bloqueo).
2. La ejecución de comandos (lanzar apps, acciones del compositor).
3. Credenciales en tránsito (PAM en M5 y cualquier secreto futuro).
4. La cadena de suministro de la compilación.

## Adversarios y superficies

| Superficie | Amenaza | Mitigación |
|---|---|---|
| Archivos `.desktop` (no confiables, escribibles por el usuario) | Ejecución de código vía `Exec` | Parsear `Exec` y usar `execvp` directo; nunca `sh -c`; fuzzear el parser (ADR-0005) |
| Temas de íconos / SVG | Exploits de parseo | `resvg`/`usvg` con límites de tamaño (M1+); fuzz |
| Stream IPC de Hyprland | Pánico/DoS por eventos malformados | Parser tipado libre de pánicos; límites de longitud |
| Servicios D-Bus (MPRIS, notificaciones, tray) | Payloads falsificados, imágenes enormes | Validar y limitar tamaño de payloads; tipos de zbus; fuzz de notificaciones |
| Archivos de configuración / paleta | Ejecución local de código, path traversal | Parseo de datos puro; sin shell; rutas resueltas sin escapes `..` |
| Red (clima, ollama) | Inyección de contenido | TLS con rustls; el renderizador trata las respuestas como datos |
| Bloqueo de pantalla (M5) | Evasión del bloqueo | Binario dedicado y mínimo, sin superficie IPC mientras está bloqueado, `ext-session-lock-v1`, PAM en una crate FFI aislada, documento de modelo de amenazas, fuzz + matriz de tests |
| Cadena de suministro | Dependencia maliciosa | `cargo-deny` (advisories, licencias, bans, sources), `cargo-audit`, lockfile fijado, CI con `--locked`, Renovate |

Excepción rastreada conocida: `ttf-parser` (RUSTSEC-2026-0192) está sin
mantenimiento pero no es vulnerable; es transitiva vía `cosmic-text → fontdb` y
se ignora en `deny.toml` hasta que esas crates migren a `skrifa`.

## Invariantes

* `unsafe` está prohibido en todo el workspace. La única excepción es el puente
  auditado de handles crudos de wgpu en `skews-render`; la FFI de PAM será la
  segunda, aislada en `skews-lock`.
* La entrada no confiable nunca llega a un shell, a `eval` ni a un format
  string.
* Los secretos (ninguno hoy; PAM en M5) nunca se loguean; los impls de `Debug`
  sobre tipos secretos se escriben a mano para redactarlos.
* Los pánicos se contienen en los límites de los actores; el kernel los
  convierte en fallas visibles en vez de corrupción silenciosa.
