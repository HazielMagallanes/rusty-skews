# Guía de traducción

> 🌐 [English](../en/translation-guide.md) · **Español**

Esta guía define cómo se traduce la documentación y dónde viven los archivos
traducidos. El inglés es la fuente de verdad; las traducciones al español deben
seguir estas reglas para que ambos árboles se mantengan sincronizados.

## 1. Dónde viven los archivos

* **Un idioma por archivo.** Nunca escribas los dos idiomas en el mismo
  documento.
* **Carpetas por idioma.** Las guías en inglés viven en `docs/en/`; las
  traducciones al español en `docs/es/`.
* **Mismo nombre de archivo en ambos árboles.** Una guía en
  `docs/en/performance.md` se traduce en `docs/es/performance.md`; no agregues
  sufijos de idioma a los nombres de las guías.
* **Archivos raíz.** `README.md`, `CONTRIBUTING.md`, `SECURITY.md` y
  `CHANGELOG.md` son las versiones canónicas en inglés (las herramientas y
  GitHub los esperan en la raíz). Sus contrapartes en español viven en
  `docs/es/` con los nombres `readme.md`, `contributing.md`, `security.md` y
  `changelog.md`.
* **Los ADR son solo en inglés.** Viven en `docs/adr/` y no se traducen; llevan
  una nota de idioma en lugar de un enlace selector.
* **Sin traducciones huérfanas.** No traduzcas un archivo que no exista en el
  otro árbol.

## 2. La línea selectora

Todo documento debe empezar con un selector de idioma inmediatamente después de
su título H1 (o de su bloque de título). Copiá uno de estos formatos tal cual:

```markdown
<!-- Documento en inglés -->
> 🌐 **English** · [Español](../es/<archivo>.md)

<!-- Documento en español -->
> 🌐 [English](../en/<archivo>.md) · **Español**

<!-- Archivo raíz en inglés (README, CONTRIBUTING, SECURITY, CHANGELOG) -->
> 🌐 **English** · [Español](docs/es/<archivo>.md)

<!-- Contraparte en español de un archivo raíz (docs/es/readme.md, …) -->
> 🌐 [English](../README.md) · **Español**

<!-- ADR (solo en inglés) -->
> 🌐 **English** · *ADRs are maintained in English only / los ADR se mantienen únicamente en inglés*
```

Reglas:

* El idioma actual va en **negrita**; el otro es un enlace relativo.
* El enlace debe apuntar al archivo contraparte, y el nombre debe seguir las
  reglas de arriba.
* Si una traducción queda desactualizada, **no quites el selector**; agregá el
  marcador de desactualizado debajo (ver §5).

## 3. Qué no traducir

Dejá exactamente igual que en la fuente en inglés:

* Código, bloques de código y código en línea: comandos, flags, rutas,
  identificadores.
* Nombres de crates, módulos, binarios y tipos (`skews-render`, `GpuSurface`,
  `Msg`).
* Nombres de protocolos y especificaciones (`wlr-layer-shell`,
  `ext-session-lock-v1`, `wl_surface.frame`, `org.freedesktop.Notifications`).
* Títulos, números y nombres de archivo de los ADR.
* Nombres propios y de productos (Hyprland, wgpu, PipeWire, Vulkan, Rust).
* Anclas de Markdown y destinos de enlaces.
* Claves de tablas, de configuración y fragmentos TOML.

## 4. Glosario

Usá el mismo término en español en todos lados. Ante la duda, preferí el
término de esta tabla antes que inventar uno nuevo.

| English | Español |
|---|---|
| bar | barra |
| shell | shell |
| widget | widget |
| panel | panel |
| layout | layout |
| frame | fotograma |
| damage (rendering) | daño |
| launcher | lanzador |
| provider | proveedor |
| wallpaper | fondo de pantalla |
| workspace | workspace |
| palette | paleta |
| config / configuration | configuración |
| binding | atajo |
| focus | foco |
| tray | bandeja |
| service | servicio |
| lock screen | pantalla de bloqueo |
| release | versión |
| budget | presupuesto |
| layout engine | motor de layout |

Si aparece un término recurrente que no está en la tabla, agregalo acá primero
y después usalo en la traducción.

## 5. Paridad y desactualización

* Mantené la estructura de títulos, tablas y bloques de código alineada entre
  ambos archivos.
* Mantené una longitud similar; no agregues consejos en un solo idioma.
* Si cambiás un documento y no podés actualizar la traducción en el mismo PR,
  marcá el archivo en español como desactualizado justo debajo del selector:

  ```markdown
  > ⚠️ **Traducción desactualizada** — pendiente de sincronizar con la versión en inglés.
  ```

* Quitá el marcador cuando la traducción se ponga al día.
* El inglés es la fuente de verdad ante cualquier conflicto.

## 6. Flujo de trabajo para cambios

1. Cambiá primero el archivo en inglés.
2. Actualizá la contraparte en español en el mismo PR, o marcala como
   desactualizada.
3. Los documentos nuevos se entregan con **ambos** archivos (salvo los ADR),
   más una fila en la tabla de documentación de `README.md` y
   `docs/es/readme.md`.
4. Al renombrar un archivo, renombralo en ambos árboles y actualizá todos los
   enlaces.
5. Las entradas del changelog se escriben primero en inglés y se traducen en la
   siguiente pasada.

## 7. Checklist de revisión para traductores

* [ ] Selector presente, con el formato correcto y el enlace resolviendo a un
      archivo existente.
* [ ] Títulos y estructura iguales a la fuente en inglés.
* [ ] Sin bloques de código, comandos, rutas o identificadores traducidos.
* [ ] Términos del glosario respetados (los nuevos se agregan a §4 en el mismo
      PR).
* [ ] Los enlaces relativos del archivo traducido apuntan a las contrapartes en
      español (por ejemplo `threat-model.md`, no `../en/threat-model.md`), salvo
      que el destino sea solo en inglés (ADRs).
* [ ] Marcador de desactualizado agregado o quitado según corresponda.
