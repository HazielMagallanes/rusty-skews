## Summary

<!-- What does this PR change and why? Link the milestone or issue. -->

## Checklist

- [ ] Tests first: the new behavior is covered by a failing-then-passing test.
- [ ] `cargo xt ci` passes locally (fmt, clippy, tests, docs, deny, audit).
- [ ] Public items documented; docs updated in **both** languages
      (`docs/en/` + `docs/es/`), with selectors intact — or the translation is
      marked outdated per the translation guide.
- [ ] `CHANGELOG.md` (and `docs/es/changelog.md` when translated) updated.
- [ ] No new `unsafe`; no new dependency cycles between layers.
- [ ] Performance-sensitive changes have a bench or a documented reason.

## Milestone

<!-- M1, M2, ... or "chore" -->
