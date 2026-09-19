# Arquitectura

> 🌐 [English](../en/architecture.md) · **Español**

> Estado: M0. Este documento describe la arquitectura objetivo; los hitos van
> entregando las piezas en orden. Los ADR registran las decisiones.

## 1. Capas (hexagonal)

Las dependencias apuntan **solo hacia abajo**:

```
L3  skews-app            kernel: Shell::update(msg) -> Effects, registro, scheduling
L3  skews-shell          binario: configuración -> registro -> kernel
L3  skews-lock, skews-ctl binarios
L2  skews-wayland        adaptador conductor: layer surfaces, salidas, asiento, frame callbacks
L2  skews-render         adaptador conductor: renderizador de escena wgpu
L2  skews-ui             elementos, estilos, animación, construcción de escena
L2  skews-text           shaping, fallback de fuentes, atlas de glifos
L2  skews-layout         integración con taffy + medición de texto
L1  skews-ipc-hyprland   adaptador conducido: eventos/acciones del compositor
L1  skews-sys-linux      adaptador conducido: sensores de /proc y /sys
L1  skews-bus            ayudas de zbus/puente de runtime
L1  skews-services.*     adaptadores conducidos: batería, audio, red, bluetooth, media, notify, tray, clima
L0  skews-core           ids, geometría, color, tiempo, Msg/Effects, puertos (sin I/O)
L0  skews-config         esquema, validación, migraciones
L0  skews-theme          tokens
L4  skews-testkit        fakes, fixtures, aserciones de escena (solo tests)
```

Las crates L0 no tienen I/O, ni Wayland, ni wgpu, ni runtime async. Eso es lo
que hace que el comportamiento del shell sea testeable sin pantalla.

## 2. Kernel: effects-as-data

```rust
fn update(&mut self, msg: Msg) -> Effects;
// Effect::Redraw | SetExclusiveZone(..) | SendService(..)
//        | RunAction(ActionId) | Persist(..) | Notify(..)
```

* Un solo escritor: el hilo principal es dueño del `State`; toda entrada
  (Wayland, servicios, timers) se convierte en un `Msg`.
* `update` es sincrónico y cuidadoso con las asignaciones; los tests alimentan
  secuencias de mensajes y verifican estado + diffs del `Scene` resultante.
* La E/S vive en actores sobre un hilo tokio dedicado, comunicados por canales
  acotados; los eventos se coalescen al último valor.
* Los pánicos de los actores se atrapan en el límite del actor; el kernel sigue
  vivo y reporta la falla. (La política de supervisión llega con `skews-app`,
  M1.)

## 3. Modelo de frames

* Los frames se construyen solo cuando hay cambios. Una barra estática dibuja
  cero frames.
* La capa de UI emite un `Scene` serializable (lista de dibujo + rectángulos de
  daño). Los tests snapshotean escenas; el renderizador las consume.
* Los callbacks `wl_surface.frame` se piden solo mientras corren animaciones.
* Cero asignaciones en régimen estacionario se verifica con el asignador
  `dhat`.

## 4. Módulos y configuración

Los módulos implementan el trait `Module` y viven en un registro en tiempo de
compilación dentro de `skews-shell` (con feature gates). La configuración
referencia ids de módulos; los errores de validación apuntan a la clave
ofensora. Los paneles los declaran los módulos y se dibujan como superficies
layer dedicadas con política centralizada de agarre/descarte. Los proveedores
del lanzador (apps, cálculo, acciones) se enchufan sin tocar el lanzador.

## 5. Hilos

```
main (calloop)        Wayland, entrada, State::update, armado de escena, wgpu
hilo de tokio         clientes/servidores zbus, IPC, HTTP, watchers de archivos
pool de rayon         decodificación de íconos/miniaturas, extracción de color
```

Sin locks en rutas calientes; canales acotados con coalescencia entre hilos.

## 6. Ayudantes externos

`awww` (demonio de fondos), `mpvpaper` (fondos de video), `matugen`
(generación de colores) y `cava` (visualizador de audio) quedan como externos y
se manejan vía adaptadores. Ya están optimizados para lo suyo; el shell se
ocupa de la composición, no de reimplementarlos.
