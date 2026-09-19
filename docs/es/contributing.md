# Contribuir

> 🌐 [English](../../CONTRIBUTING.md) · **Español**

Gracias por ayudar a construir rusty-skews. Este proyecto se desarrolla con un
flujo TDD estricto y con presupuestos de seguridad y rendimiento revisables.

Al participar, aceptás el [Código de conducta](code-of-conduct.md).

## Reglas básicas

1. **Primero los tests.** Todo cambio de comportamiento empieza como un test
   que falla en la crate dueña de la lógica (ver
   [docs/es/testing.md](testing.md)).
2. **Las capas apuntan hacia abajo.** `skews-core` nunca depende de los
   adaptadores; los adaptadores nunca dependen del núcleo. Agregar una
   dependencia que viole [docs/es/architecture.md](architecture.md) requiere un
   ADR.
3. **Nada de `unsafe`.** Los lints del workspace lo prohíben. La única excepción
   es el puente auditado de superficies wgpu en `skews-render`; la FFI de PAM
   quedará igualmente aislada en `skews-lock`.
4. **Nada de interpolar en un shell.** Nunca pases datos del usuario por
   `sh -c`; parseá y usá `execvp`, o hablá directo con los protocolos (ver
   [ADR-0005](../adr/0005-no-shell-interpolation.md)).
5. **Conventional Commits** (`feat:`, `fix:`, `perf:`, `docs:`, `chore:`,
   `security:`…). Commits enfocados.
6. **La documentación se mueve con el código.** Actualizá el README, las tablas
   de docs y los ADRs en el mismo PR.

## Documentación y traducciones

La documentación es **un idioma por archivo**, organizada en carpetas por
idioma:

* Las guías en inglés viven en `docs/en/`; las traducciones al español en
  `docs/es/`.
* Los archivos raíz (`README.md`, `CONTRIBUTING.md`, `SECURITY.md`,
  `CHANGELOG.md`) son las versiones canónicas en inglés; sus contrapartes en
  español viven en `docs/es/` (`readme.md`, `contributing.md`, `security.md`,
  `changelog.md`).
* Todo documento empieza con una línea selectora de idioma que enlaza a su
  contraparte.
* Los ADRs (`docs/adr/`) se mantienen únicamente en inglés.

Cuando agregues o cambies un documento, seguí la
[guía de traducción](translation-guide.md): actualizá la fuente en inglés y
actualizá el archivo en español en el mismo PR o marcalo como desactualizado.

## Puerta local

> ¿Primera vez? Empezá por la
> [guía de primeros pasos](getting-started.md).

```sh
cargo xt ci          # fmt + clippy + tests + doc + deny + audit
cargo xt fmt         # solo formato
cargo xt test        # nextest si está instalado; si no, cargo test
cargo xt demo        # ejecuta el shell 5s contra la sesión real
```

## Agregar un módulo (desde M1)

Los módulos son la unidad de extensión. La receta:

1. Creá el widget/servicio en `skews-modules` (separalo a su propia crate
   cuando crezca).
2. Implementá `Module` (id, init, update, bar_view/panels/launcher opcionales).
3. Registralo en el registro en tiempo de compilación de `skews-shell`.
4. Agregá sus valores por defecto al esquema de configuración si tiene opciones.
5. Entregá tests: tests unitarios de lógica pura, un test de validación de
   configuración y — si dibuja — un snapshot de `Scene`.

El bucle del núcleo no debe cambiar por módulos nuevos; si cambia, es un ADR.

## Checklist de revisión

* [ ] Los tests cubren el comportamiento nuevo y el anterior sigue cubierto.
* [ ] `cargo xt ci` pasa localmente.
* [ ] Los ítems públicos están documentados; docs/ADRs actualizados.
* [ ] Los documentos nuevos/cambiados tienen selector y contraparte traducida
      (o están marcados como desactualizados).
* [ ] Sin `unsafe` nuevo; sin ciclos de dependencia nuevos.
* [ ] El código sensible al rendimiento tiene un bench de criterion o una razón
      documentada.
