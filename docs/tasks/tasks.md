---
title: Tasks
description: Deferred work on thorn, one file each — a commitment with a done state
author: adammharris
created: 2026-09-19
updated: 2026-09-26
contents:
- '[The wasm binding, for the composer](wasm-binding.md)'
- '[Thorn on the App Store, through Xcode Cloud](sandboxed-mac-app.md)'
- '[UniFFI 0.32 needs a resvg-swift release](uniffi-032-needs-a-resvg-release.md)'
- '[Closed tasks](/docs/tasks/closed/closed.md)'
part_of: '[thorn](/README.md)'
---
# Tasks

Work that has been deferred, not work that is planned. A task lands here when
it is worth doing and will not be done in the commit that noticed it; a fix
that takes ten minutes gets a commit, not a file. Each carries `status`
(`open`, `in-progress`, `done`, `dropped`) in its frontmatter — `dx tasks`
reads the key — and closing one is an edit, naming the commit or release
that resolved it, never a deletion. What is done keeps its place in the list
above — the index is the spine, and what is open is a view of it (`dx tasks`)
— and stays findable by grep.

What this is not: a commitment to consumers, which goes in the changelog's
unreleased region; or a description of what shipped, which goes in the
READMEs and `docs/profile.md`.
