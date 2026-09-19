# rusty-skews

> 🌐 [English](../../README.md) · **Español**

Un shell de escritorio para Wayland, nativo en Rust y con identidad *skewed*,
pensado para Hyprland (y otros compositores con layer-shell), construido para
**rendimiento, seguridad y escalabilidad**.

> Inspirado en la UI/UX de [skwd](https://github.com/HazielMagallanes/skwd),
> pero arquitecturado desde cero: kernel hexagonal por capas, modelo de estado
> *effects-as-data*, composición de módulos por configuración y un renderizador
> GPU bajo demanda.

**Estado:** M0 — andamiaje + barra “hola mundo” (una barra temática con
layer-shell se dibuja en cada salida). Ver la tabla de hitos más abajo.

---

## Arquitectura en resumen

```
L3  skews-app / skews-shell        kernel: update(msg) -> Effects, registro, binarios
L2  skews-wayland, skews-render    adaptadores conductores (layer surfaces, wgpu)
L2  skews-ui, skews-text, skews-layout
L1  skews-services, skews-ipc-hyprland, skews-sys-linux, skews-bus
L0  skews-core, skews-config, skews-theme   (sin I/O, trivialmente testeable)
```

* **Effects-as-data**: `update()` muta el estado y devuelve comandos; el
  comportamiento se testea sin pantalla.
* **Frames bajo demanda**: el renderizador dibuja solo cuando cambia el estado.
* **Composición por configuración**: las regiones y módulos de la barra viven
  en TOML; agregar un widget nunca toca el núcleo.
* **`#![forbid(unsafe_code)]`** en todo el workspace; el único bloque `unsafe`
  auditado es la creación de superficies wgpu en `skews-render`.

Leer [docs/es/architecture.md](architecture.md) para los detalles y
[docs/adr/](../adr) para las decisiones.

---

## Inicio rápido

> ¿Primera vez? La [guía de primeros pasos](getting-started.md) recorre
> requisitos, la demo, la configuración y la solución de problemas.

Requisitos: Rust 1.98+, una sesión Wayland (se recomienda Hyprland), Vulkan o
GL, y `libwayland` (se usa el backend `system` para entregar superficies a wgpu).

```sh
# Diagnóstico: verifica Wayland, compositor, GPU, configuración y tema.
cargo run -p skews-ctl -- doctor

# Ejecutar el shell (dibuja una barra temática en cada salida).
cargo run -p skews-shell

# Puerta de CI local completa (fmt, clippy, tests, docs, deny, audit).
cargo xt ci
```

La configuración por defecto vive en `$XDG_CONFIG_HOME/rusty-skews/config.toml`
y la paleta en `$XDG_CACHE_HOME/rusty-skews/colors.json` (estilo matugen, plano
`token -> "#rrggbb"`). Si faltan, se usan los valores internos.

```toml
[bar]
height = 32
monitor = "*"

[bar.left]
modules = ["workspaces", "media"]
[bar.center]
modules = ["clock"]
[bar.right]
modules = ["cpu", "memory", "battery", "network", "audio", "tray"]
```

---

## Hitos

| Hito | Alcance | Estado |
|---|---|---|
| M0 | Andamiaje, CI, ADRs, doctor, barra en todas las salidas | ✅ |
| M1 | Kernel + núcleo de UI (texto, layout, render, ui) + módulos de barra | ⏳ |
| M2 | Paneles, servicios (audio/red/bluetooth/media/notify/tray), OSD | ⏳ |
| M3 | Lanzador con proveedores (apps, cálculo, acciones) | ⏳ |
| M4 | Selector de fondos y pipeline de temas | ⏳ |
| M5 | Conmutador de ventanas y bloqueo de sesión (PAM) | ⏳ |
| M6 | Ajustes y empaquetado (v0.1) | ⏳ |

## Estructura del workspace

```
crates/
  skews-core      tipos, Msg/Effects, puertos (sin I/O)
  skews-config    esquema TOML, validación, rutas
  skews-theme     tokens desde JSON de matugen + valores por defecto
  skews-ipc-hyprland  eventos tipados del compositor
  skews-sys-linux     lectores de /proc y /sys (coretemp + k10temp)
  skews-bus           ayudas de D-Bus (M1+)
  skews-services      adaptadores de servicios (M1+)
  skews-text          shaping + atlas de glifos (M1+)
  skews-layout        integración con taffy (M1+)
  skews-render        renderizador wgpu (SDF hoy, batcher en M1)
  skews-ui            elementos/estilos/animación/escena (M1+)
  skews-wayland       pegamento de layer-shell
  skews-app           kernel + registro (M1)
  skews-modules       módulos integrados (M1+)
  skews-shell         binario `rusty-skews`
  skews-lock          binario `rusty-skews-lock` (M5)
  skews-ctl           binario `rusty-skews-ctl` (doctor hoy)
  skews-testkit       fakes, fixtures, aserciones de escena (M1+)
xtask/  docs/  tests/  benches/
```

## Documentación

| Documento | English | Español |
|---|---|---|
| Primeros pasos | [en](../en/getting-started.md) | [es](getting-started.md) |
| Arquitectura | [en](../en/architecture.md) | [es](architecture.md) |
| Presupuestos de rendimiento | [en](../en/performance.md) | [es](performance.md) |
| Estrategia de testing | [en](../en/testing.md) | [es](testing.md) |
| Modelo de amenazas | [en](../en/threat-model.md) | [es](threat-model.md) |
| Guía de traducción | [en](../en/translation-guide.md) | [es](translation-guide.md) |
| Decisiones (ADRs) | [solo en inglés](../adr) | — |

Ver [CONTRIBUTING (ES)](contributing.md) para el flujo de trabajo y
[SECURITY (ES)](security.md) para reportar vulnerabilidades.

## Licencia

MIT — ver [LICENSE](../../LICENSE).
