# Política de seguridad

> 🌐 [English](../../SECURITY.md) · **Español**

## Reportar una vulnerabilidad

Reportá en privado vía GitHub Security Advisories en
`HazielMagallanes/rusty-skews`, o por correo a
contactame.haziel@gmail.com. Incluí una reproducción y el hito/crate afectado.
Apuntamos a acusar recibo dentro de 72 horas.

**No** abras issues públicos para problemas explotables.

## Alcance

rusty-skews es un cliente Wayland que corre como tu usuario y habla con el
compositor, servicios D-Bus y la red en tu nombre. Son de alcance, por ejemplo:

* El parseo de entradas no confiables: archivos `.desktop`, temas de íconos,
  eventos IPC de Hyprland, payloads de notificaciones, configuración y temas
  (todos objetivos de fuzzing).
* La ejecución de comandos: lanzar apps nunca debe pasar por un shell; un
  `Exec` de `.desktop` que pueda escapar a interpretación de shell es una
  vulnerabilidad (ver [ADR-0005](../adr/0005-no-shell-interpolation.md)).
* El bloqueo de sesión (`skews-lock`, M5): evadir la superficie de bloqueo,
  exponer IPC mientras está bloqueado, uso indebido de PAM o desincronización
  del estado de bloqueo.
* Cadena de suministro: comportamiento inesperado de red/archivos por parte de
  dependencias.

## Garantías base

* `#![forbid(unsafe_code)]` en todo el workspace; la única excepción es el
  puente auditado de handles crudos de wgpu en `skews-render`.
* No se loguean secretos; el parseo de paleta/configuración nunca ejecuta
  contenido.
* `cargo-deny` + `cargo-audit` corren en CI; las dependencias nuevas se revisan
  por licencia, mantenimiento y uso de `unsafe`.

## Modelo de amenazas

Ver [docs/es/threat-model.md](threat-model.md) para el modelo completo,
incluido el análisis de amenazas del bloqueo de pantalla que llega en M5.
