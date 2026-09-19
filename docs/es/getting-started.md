# Primeros pasos

> 🌐 [English](../en/getting-started.md) · **Español**

Todo lo necesario para pasar de un clon nuevo a ver rusty-skews corriendo en tu
propia sesión, en unos diez minutos.

## 1. Requisitos

| Requisito | Notas |
|---|---|
| Rust | El toolchain fijado (1.98.1) está en `rust-toolchain.toml`; rustup lo instala solo en la primera compilación. |
| Sesión Wayland | Hyprland es el objetivo principal. Cualquier compositor wlroots/smithay con `wlr-layer-shell` sirve; GNOME no lo soporta. |
| GPU con Vulkan | Mesa o drivers del fabricante. GL funciona como respaldo; `rusty-skews-ctl doctor` te dice qué se detectó. |
| Dependencias de build | `libwayland`, `pkg-config` y un toolchain de C. |

Ejemplo para Arch:

```sh
sudo pacman -S --needed base-devel pkgconf libwayland
```

## 2. Compilar y verificar

```sh
cd ~/codigos/personal/rust/rusty-skews

# Puerta local completa: fmt, clippy, tests, docs, deny, audit.
cargo xt ci

# Chequeo de entorno: Wayland, compositor, GPU, configuración, tema.
cargo run -p skews-ctl -- doctor
```

`doctor` sale con código distinto de cero cuando falta algo requerido; agregá
`--strict` para tratar los avisos (sin configuración/tema, compositor
desconocido) como fallas.

## 3. Correr la demo

La demo ejecuta el shell real contra tu sesión viva unos segundos y sale sola:

```sh
cargo xt demo              # 5 segundos
cargo xt demo --seconds 10 # una mirada más larga
```

Lo que deberías ver:

* Una barra de 32 px anclada arriba en **cada salida**.
* Fondo con el color `surface_container` del tema al 90% de alfa y un pequeño
  punto central del color `primary` (marcador de posición de M0).
* Mientras corre, la barra reserva una zona exclusiva, así que los layouts
  hacen lugar; al salir, libera la zona.

Ejecución manual equivalente (la demo es un envoltorio de esto):

```sh
cargo run -p skews-shell -- --exit-after 6
```

Flags útiles de `skews-shell`: `-c/--config <ruta>`, `-v` (repetible para más
logs), `--exit-after <segundos>` (ayuda de desarrollo).

> La barra de M0 solo dibuja; todavía no hay módulos interactivos (reloj,
> workspaces y sensores llegan en M1).

## 4. Probarlo como tu shell (opcional)

Sigue siendo un andamiaje M0, así que tratalo como una prueba. Si usás otro
shell (por ejemplo la configuración de skwd), recordá que ambas barras
reservarían espacio a la vez.

```sh
# Detener el shell Wayland que esté corriendo (ejemplo: skwd basado en quickshell).
pkill -f 'quickshell -p /usr/share/skwd'

# Correr rusty-skews en primer plano; Ctrl+C para salir.
cargo run -p skews-shell

# Restaurar el shell anterior (ejemplo).
setsid env QT_QUICK_BACKEND=software skwd >/dev/null 2>&1 &
```

## 5. Configuración

Ambos archivos son opcionales; si faltan se usan los valores internos y
`doctor` reporta un aviso.

* Configuración: `$XDG_CONFIG_HOME/rusty-skews/config.toml` (normalmente
  `~/.config/rusty-skews/config.toml`).

  ```toml
  [bar]
  height = 32
  monitor = "*"          # o "eDP-1", "HDMI-A-1", ...

  [bar.left]
  modules = ["workspaces"]
  [bar.center]
  modules = ["clock"]
  [bar.right]
  modules = ["cpu", "memory", "battery", "network", "audio", "tray"]
  ```

  Los módulos referenciados se validan contra el registro en M1; hoy la barra
  dibuja el marcador de posición igual.

* Paleta: `$XDG_CACHE_HOME/rusty-skews/colors.json` (normalmente
  `~/.cache/rusty-skews/colors.json`), un mapa plano estilo matugen:

  ```json
  { "primary": "#89b4fa", "surface_container": "#313244", "on_surface": "#cdd6f4" }
  ```

  Las claves desconocidas se ignoran; los colores inválidos se reportan por
  nombre.

## 6. Solución de problemas

| Síntoma | Solución |
|---|---|
| `doctor` falla en `wayland` | Ejecutá desde la sesión gráfica, no por SSH; revisá `WAYLAND_DISPLAY`. |
| `doctor` falla en `gpu` | Instalá un driver Vulkan (`mesa`, ICDs del fabricante) o probá `WGPU_BACKEND=gl`. |
| Error de build sobre `wayland-sys`/`libwayland` | Instalá `libwayland` y `pkg-config`. |
| El shell sale con "failed to bind bar surfaces" | El compositor no tiene `wlr-layer-shell` (p. ej. GNOME); usá Hyprland u otro compositor wlroots/smithay. |
| La barra no se ve | La demo sale a los N segundos; probá `--seconds 15` o corré `cargo run -p skews-shell` con `-vv` y mirá los logs. |

## 7. A dónde ir después

* [Arquitectura](architecture.md) — cómo encajan el kernel, los módulos y el
  renderizador.
* [Testing](testing.md) — las seis clases de test y cómo correrlas.
* [Rendimiento](performance.md) — presupuestos y cómo se miden.
* [Contribuir](contributing.md) — flujo de trabajo y cómo agregar un módulo
  (M1+).
* [Hitos](readme.md#hitos) — qué se construye después.
