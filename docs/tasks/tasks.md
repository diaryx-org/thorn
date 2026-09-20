---
title: Tasks
description: Deferred work on svg-editor, one file each — a commitment with a done state
author: adammharris
created: 2026-09-19
updated: 2026-09-20
contents:
- '[First release](first-release.md)'
- '[Move, resize and reorder wait on twig''s drawing branch](gestures-need-twig.md)'
- '[Deleting a shape leaves its indentation line behind](delete-leaves-its-line.md)'
- '[Hit-testing and selection in the core](hit-testing.md)'
- '[The canvas view, on the Mac and on iOS](swift-canvas.md)'
- '[The style block a new drawing is created with](style-template.md)'
- '[The wasm binding, for the composer](wasm-binding.md)'
part_of: '[svg-editor](/README.md)'
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
