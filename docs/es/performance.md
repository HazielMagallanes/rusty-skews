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

## Cómo se hacen cumplir

* **Diseño**: frames bajo demanda, seguimiento de daño, reutilización de
  glifos/quads, sin bucles de sondeo salvo sensores (intervalos ≥ 1 s),
  coalescencia de eventos.
* **Tests**: aserciones de cero asignaciones con `dhat` sobre la ruta
  estacionaria; benches de criterion para lógica pura caliente (parseo de
  configuración, ranking, layout); benches de armado de `Scene`.
* **Medición local**: `cargo xt bench-live` (M1) reporta RSS y tiempos de frame
  desde `/proc` y los timestamps de wgpu. Los números de CI son indicativos; la
  aceptación se corre en la máquina objetivo.

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
