---
title: The Mac app runs unsandboxed and unsigned
description: Thorn.app builds and runs where it was built; signing, the sandbox and notarisation stand between it and a build anyone else can open
author: adammharris
status: open
created: 2026-09-21
updated: 2026-09-21
part_of: '[Tasks](tasks.md)'
---
# The Mac app runs unsandboxed and unsigned

**Where.** `apps/thorn-editor/project.yml`, and a packaging step in `xtask`.

**What.** Thorn is a document app now — `DocumentGroup`, a declared document
type, an icon, a marketing version — but `ENABLE_APP_SANDBOX` is off and
`CODE_SIGNING_ALLOWED` is `NO`, so `cargo xtask swift --release` produces an
app that runs on the machine that built it and nowhere else. Turning either
on has a consequence the build does not answer yet:

- **Signing** needs a Developer ID, and a distributable build needs
  notarisation on top; both are Adam's to run. The build should take the
  team and identity from the environment (`DEVELOPMENT_TEAM`,
  `CODE_SIGN_IDENTITY`) rather than the project, so a checkout without them
  still builds.
- **The sandbox** grants a document app the file it was handed, and a
  drawing of the profile is one file — its style, its markers, its ink are
  all inside the `<svg>`. So unlike leaf's (`leaf/docs/tasks/sandboxed-mac-app.md`),
  this sandbox has no sibling-file question; an `<image href>` to a file
  beside the drawing is the one thing it would lose, and the profile does not
  offer one. macOS will not launch a sandboxed app that is not signed, which
  is why the two are one task.

**Done when.** `cargo xtask package` (or `swift --release --sign`) produces a
signed, sandboxed, notarised `Thorn.app` in a `.dmg`, and a drawing opened
from the Finder edits and saves.
