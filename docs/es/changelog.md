# Registro de cambios

> 🌐 [English](../../CHANGELOG.md) · **Español**

Todos los cambios notables de este proyecto se documentan acá. El formato sigue
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) y el proyecto adhiere a
[Versionado Semántico](https://semver.org/spec/v2.0.0.html) cuando publique
versiones.

> Las entradas nuevas se escriben primero en inglés; el registro en español se
> actualiza en la siguiente pasada de traducción (ver la
> [guía de traducción](translation-guide.md)).

## [Sin publicar]

### Agregado

* Andamiaje M0: workspace de 19 crates con límites de capas estrictos y
  `forbid(unsafe_code)`; binarios `rusty-skews`, `rusty-skews-ctl` y
  `rusty-skews-lock`.
* `skews-core`: geometría con matemática de intersección/unión apta para daño
  (*damage*), parseo de hex RGBA, vocabulario `Msg`/`Effect` (tipos del kernel
  *effects-as-data*).
* `skews-config`: esquema TOML versionado con validación (altura de barra,
  módulos duplicados, ids vacíos), valores por defecto y resolución de rutas
  XDG.
* `skews-theme`: ingesta de paleta estilo matugen con paleta de respaldo segura.
* `skews-ipc-hyprland`: parseo tipado y libre de pánicos de los eventos IPC de
  Hyprland.
* `skews-sys-linux`: lector de temperatura por hwmon que soporta **Intel
  (coretemp)** y AMD (k10temp), más lectores de memoria y uso de CPU con
  fixtures.
* `skews-wayland` + `skews-render`: superficies de barra layer-shell en cada
  salida con un pipeline wgpu de rectángulos redondeados SDF (bajo demanda; un
  frame por configuración).
* `rusty-skews-ctl doctor`: chequeos de entorno (Wayland, compositor, GPU,
  configuración, tema) con `--strict`.
* Documentación: arquitectura, presupuestos de rendimiento, estrategia de
  testing, modelo de amenazas, ADRs 0001–0005; workflow de CI (fmt, clippy,
  tests, docs, deny, audit).
* La documentación se divide en `docs/en/` y `docs/es/` con selectores de idioma
  por archivo y una guía de traducción para contribuyentes.
* Onboarding: guía de primeros pasos (EN/ES) con requisitos, la demo, la
  configuración y solución de problemas.
* Archivos de comunidad para el repositorio publicado: Código de conducta
  ([Contributor Covenant 2.1](../../CODE_OF_CONDUCT.md), EN/ES), plantillas de
  issues (reporte de bug, pedido de feature), plantilla de pull request y
  configuración de Dependabot.
