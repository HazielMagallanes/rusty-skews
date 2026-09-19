# Translation guide

> 🌐 **English** · [Español](../es/translation-guide.md)

This guide defines how documentation is translated and where translated files
live. English is the source of truth; Spanish translations must follow these
rules so the two trees stay in sync.

## 1. Where files live

* **One language per file.** Never write both languages in the same document.
* **Language folders.** English guides live in `docs/en/`; Spanish translations
  live in `docs/es/`.
* **Same filename in both trees.** A guide at `docs/en/performance.md` is
  translated at `docs/es/performance.md`; do not add language suffixes to guide
  filenames.
* **Root files.** `README.md`, `CONTRIBUTING.md`, `SECURITY.md` and
  `CHANGELOG.md` are the English canonical versions (tools and GitHub expect
  them at the root). Their Spanish counterparts live in `docs/es/` with the
  names `readme.md`, `contributing.md`, `security.md` and `changelog.md`.
* **ADRs are English-only.** They live in `docs/adr/` and are not translated;
  they get a language note instead of a selector link.
* **No orphan translations.** Do not translate a file that does not exist in
  the other tree.

## 2. The selector line

Every document must start with a language selector immediately after its H1
title (or after the title block). Copy one of these formats exactly:

```markdown
<!-- English document -->
> 🌐 **English** · [Español](../es/<file>.md)

<!-- Spanish document -->
> 🌐 [English](../en/<file>.md) · **Español**

<!-- Root English file (README, CONTRIBUTING, SECURITY, CHANGELOG) -->
> 🌐 **English** · [Español](docs/es/<file>.md)

<!-- Root Spanish counterpart (docs/es/readme.md, …) -->
> 🌐 [English](../README.md) · **Español**

<!-- ADR (English-only) -->
> 🌐 **English** · *ADRs are maintained in English only / los ADR se mantienen únicamente en inglés*
```

Rules:

* The current language is **bold**; the other one is a relative link.
* The link must point to the counterpart file, and the filename must match the
  naming rules above.
* If a translation falls behind, **do not remove the selector**; add the
  outdated marker below it (see §5).

## 3. What not to translate

Leave these exactly as in the English source:

* Code, code fences and inline code: commands, flags, paths, identifiers.
* Crate, module, binary and type names (`skews-render`, `GpuSurface`, `Msg`).
* Protocol and spec names (`wlr-layer-shell`, `ext-session-lock-v1`,
  `wl_surface.frame`, `org.freedesktop.Notifications`).
* ADR titles, numbers and file names.
* Proper nouns and product names (Hyprland, wgpu, PipeWire, Vulkan, Rust).
* Markdown anchors and link targets.
* Table keys, configuration keys and TOML snippets.

## 4. Glossary

Use the same Spanish term everywhere. When in doubt, prefer the term this table
uses over inventing a new one.

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

If a new recurring term appears that is not in this table, add it here first,
then use it in the translation.

## 5. Parity and drift

* Keep heading structure, tables and code blocks aligned between the two files.
* Keep the same file length within reason; do not add advice only in one
  language.
* If you change a document and cannot update the translation in the same PR,
  mark the Spanish file as outdated right under the selector:

  ```markdown
  > ⚠️ **Traducción desactualizada** — pendiente de sincronizar con la versión en inglés.
  ```

* Remove the marker when the translation catches up.
* English is the source of truth in any conflict.

## 6. Workflow for changes

1. Change the English file first.
2. Update the Spanish counterpart in the same PR, or mark it outdated.
3. New documents ship with **both** files (except ADRs), plus a row in the
   documentation table in `README.md` and `docs/es/readme.md`.
4. When renaming a file, rename it in both trees and update every link.
5. Changelog entries are written in English first and translated on the next
   translation pass.

## 7. Review checklist for translators

* [ ] Selector present, correct format, link resolves to an existing file.
* [ ] Headings and structure match the English source.
* [ ] No translated code blocks, commands, paths or identifiers.
* [ ] Glossary terms respected (add new terms to §4 in the same PR).
* [ ] Relative links in the translated file point to Spanish counterparts
      (e.g. `threat-model.md`, not `../en/threat-model.md`) unless the target
      is English-only (ADRs).
* [ ] Outdated marker added or removed as appropriate.
