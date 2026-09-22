---
title: First release
description: What stands between this checkout and a 0.1.0 the app can pin — the repo, the token, the number
author: adammharris
status: done
created: 2026-09-19
updated: 2026-09-22
part_of: '[Tasks](tasks.md)'
---
# First release

What stands between this checkout and a `0.1.0` the Diaryx app can pin:

- The GitHub repository `diaryx-org/thorn` — `gh repo create`, then
  push. `dx clone --check` reports it missing until then.
- The `CARGO_REGISTRY_TOKEN` secret on the repo, with `publish-new` scope —
  all three crates are new to crates.io. `publish.yml` names it.
- ~~The name.~~ Decided 2026-09-20: `thorn`, with the crates `thorn-svg`,
  `thorn-svg-core` and `thorn-svg-ffi` because `thorn` is held on crates.io.
- The version. `dx release` with no spec proposes; the number is Adam's.
- Then the app: diaryx's branch `thorn` builds against this checkout
  through `~/diaryx/.cargo/patches.toml` and a `path:` on the `Thorn`
  package; once 0.1.0 is on crates.io and tagged, its pins flip to the
  version (`thorn-svg-ffi = "0.1"` resolving from the registry,
  `Thorn: url … exactVersion: 0.1.0`), `Thorn` joins the app's
  acknowledgements, and the branch merges.
- Then `dx release <spec>` cuts bump, changelog, commit, and tag, and the tag
  push runs `publish.yml`.

**Resolved.** `v0.1.0` is tagged and `thorn-svg`, `thorn-svg-core` and
`thorn-svg-ffi` are on crates.io (0.1.1 since); the app pins
`thorn-svg-ffi = "0.1"` and `Thorn` at `exactVersion: 0.1.0`, lists it in
its acknowledgements, and its `thorn` branch has merged.
