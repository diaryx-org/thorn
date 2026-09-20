---
title: First release
description: What stands between this checkout and a 0.1.0 the app can pin — the repo, the token, the number
author: adammharris
status: open
created: 2026-09-19
updated: 2026-09-19
part_of: '[Tasks](tasks.md)'
---
# First release

What stands between this checkout and a `0.1.0` the Diaryx app can pin:

- The GitHub repository `diaryx-org/svg-editor` — `gh repo create`, then
  push. `dx clone --check` reports it missing until then.
- The `CARGO_REGISTRY_TOKEN` secret on the repo, with `publish-new` scope —
  all three crates are new to crates.io. `publish.yml` names it.
- The name. `svg-editor` is a placeholder; the org's proposal argued
  `petal`. Renaming touches the crate names, the Swift module names, the
  generated binding, `repos.figl`, and the workspace name in
  `.config/prov.yaml` — do it before the first tag, because a crates.io
  name is forever.
- The version. `dx release` with no spec proposes; the number is Adam's.
- Then `dx release <spec>` cuts bump, changelog, commit, and tag, and the tag
  push runs `publish.yml`.
