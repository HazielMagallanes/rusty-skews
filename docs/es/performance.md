# Rendimiento

> 🌐 [English](../en/performance.md) · **Español**

El shell apunta a un rendimiento de escritorio real: costo nulo en reposo,
latencia de frame acotada y cero rotación de asignaciones.

## Presupuestos

| Métrica | Presupuesto |
|---|---|
| CPU en reposo (barra visible, sin animación) | < 0.2 % de un núcleo |
| Arranque en frío → primer frame de la barra | < 150 ms (objetivo 80 ms) |
| RSS en reposo (solo barra) | < 60 MB |
| RSS con caché del lanzador de 3k apps | < 120 MB |
| CPU por frame de barra estática | < 1 ms |
| CPU por frame de animación a 120 Hz | < 4 ms |
| Apertura del lanzador (caché caliente) | < 16 ms |
| Reconstrucción de caché de apps (3k entradas) | < 50 ms |
| Asignaciones de heap por frame en régimen estacionario | 0 |

## Medido (aceptación M1, 2026-09-25)

Máquina de desarrollo: Intel Iris Xe (Vulkan), Hyprland, 1920×1080 a escala 1,
build release, ventana de 60 s más un soak de 15 min.

| Métrica | Medido | Presupuesto |
|---|---|---|
| Arranque en frío → primer frame | 81 ms (corrida de 60 s) / 91 ms (soak) | < 150 ms |
| CPU en reposo | 0.12 % (60 s) / 0.11 % (soak de 15 min) | < 0.2 % |
| RSS en reposo | 31.6 MiB (60 s) / 34.4 MiB tras 15 min, sin deriva | < 60 MiB |

Hallazgos de la corrida de aceptación:

* La instancia de wgpu debe pedir **solo Vulkan**: `Backends::all()` también
  inicializa el driver GL, que mapea más de 100 MiB extra en Mesa. GL queda
  como respaldo cuando no hay adaptador Vulkan.
* Reconfigurar el swapchain es caro; las superficies se reconfiguran solo
  cuando cambia su tamaño.
* Reloj y sensores tickean cada 2 s (`TICK_INTERVAL` en `skews-shell`), lo que
  mantiene la CPU en reposo bien dentro del presupuesto. Los intervalos por
  módulo llegan en M2.
* Las cero asignaciones en régimen estacionario (ADR-0002) **aún no se
  verifican**: el constructor de escenas todavía asigna en cada redibujado. La
  reutilización con arenas está planificada para el slice de endurecimiento del
  núcleo de UI.

## Cómo se hacen cumplir

* **Diseño**: frames bajo demanda, seguimiento de daño, reutilización de
  glifos/quads, sin bucles de sondeo salvo sensores (intervalos ≥ 1 s),
  coalescencia de eventos.
* **Tests**: aserciones de cero asignaciones con `dhat` sobre la ruta
  estacionaria; benches de criterion para lógica pura caliente (parseo de
  configuración, ranking, layout); benches de armado de `Scene`.
* **Medición local**: `cargo xt bench-live` compila el shell en release, espera
  el primer frame y reporta arranque en frío, CPU en reposo y RSS contra los
  presupuestos documentados (`--seconds` para corridas de soak). Los números de
  CI son indicativos; la aceptación se corre en la máquina objetivo.

## Perfiles de compilación

| Perfil | Propósito | Ajustes clave |
|---|---|---|
| `dev` | iteración | por defecto |
| `release` | distribución | `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip`, `opt-level=3` |
| `release-debug` | profiling | release + debuginfo |
| `bench` | criterion | hereda release (con unwinding) |

Las compilaciones locales pueden agregar `-C target-cpu=native`; los artefactos
de release para otras máquinas no deben hacerlo.

## Notas de metodología

* Medí con el compositor en una escena realista (barra + un panel abierto), al
  menos 60 s después del arranque para que las cachés estén calientes.
* Reportá: RSS pico y estacionario, CPU en reposo por 5 minutos, CPU por frame
  p50/p95 mientras anima, latencia del primer frame desde el inicio del proceso.
* Las regresiones de más del 20 % en cualquier presupuesto requieren una nota
  explicativa (ADR) en el PR.
