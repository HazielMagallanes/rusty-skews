# Testing

> 🌐 [English](../en/testing.md) · **Español**

TDD es el modo de trabajo: todo cambio de comportamiento empieza como un test
que falla.

## Clases de test

| # | Clase | Herramientas | Qué cubre |
|---|---|---|---|
| 1 | Unidad + propiedades | `cargo test`, `proptest` (M1+) | Lógica pura: geometría, parsers, validación de configuración, ranking, matemática de layout |
| 2 | Snapshots | `insta` (M1+) | Listas de dibujo del `Scene`, serialización de configuración, caché de apps, ranking de búsqueda |
| 3 | Integración D-Bus | `dbus-run-session` + fakes de `skews-testkit` | Servicios: batería, MPRIS, notificaciones, tray |
| 4 | Integración Wayland | `labwc`/`sway` con `WLR_BACKENDS=headless` | Ciclo de vida de layer surfaces, hotplug de salidas, paneles |
| 5 | Goldens de GPU | wgpu sobre Mesa **lavapipe** (`vulkan-swrast`) | Render offscreen + comparación de imágenes por SSIM |
| 6 | Rendimiento | `criterion`, `dhat` | Benches de rutas calientes y aserciones de cero asignaciones |

Los tests deterministas nunca dependen de la sesión viva del desarrollador; la
sesión viva solo se usa desde `cargo xt demo`.

## Cómo correrlos

```sh
cargo xt test                 # nextest si está instalado; si no, cargo test
cargo test -p skews-core      # una sola crate
cargo xt demo                 # corrida de humo en vivo por 5 segundos
```

Las fixtures viven junto a la crate dueña de la lógica de parseo
(`crates/<crate>/tests/fixtures/`) y se referencian vía `CARGO_MANIFEST_DIR`,
así los tests corren desde cualquier directorio.

## Política

* Nada de tests inestables en CI. Un test que no puede hacerse determinista se
  marca `#[ignore = "razón"]` y se rastrea.
* Los tests de integración que necesitan un demonio de prueba (D-Bus,
  compositor headless) se gatean por detección de entorno y fallan con un
  mensaje accionable.
* Los cambios de snapshots se revisan como código: un diff de `insta` en un PR
  es un cambio de comportamiento y debe justificarse.
