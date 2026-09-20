---
title: Freehand ink in the core
description: 'Drawing::add_path over data-ink, the term the profile reserves: a centreline from the pointer in, an outline path out, under the nib rules the org proposal leaves to settle'
author: adammharris
status: open
created: 2026-09-20
updated: 2026-09-20
part_of: '[Tasks](tasks.md)'
---
# Freehand ink in the core

The toolbar has a draw tool (`P` / `7`, `Tool.draw`) and it is disabled.
The profile reserves `data-ink` for it — a `<path>` whose `d` is an
outline the nib computed, filled — and points at the ink section of the
org's proposal (`~/diaryx/docs/proposals/drawing-editor-repository.md`,
"Freehand ink"), which leaves the nib to settle: whether the outline is
PencilKit's on Apple and the core's own elsewhere, and whether the file
carries the centreline beside it.

**Settled 2026-09-20** (recorded in the org proposal's "To settle"): the
core owns the nib model — centreline and widths in, outline out — so
there is one writer whatever the stroke came from, and a host's outline
(PencilKit's) is input it may hand over; the file carries
`data-centreline` and `data-widths` beside the outline; the first and,
for now, only nib is monoline, and the profile admits only the nibs the
core draws. What the tool needs:

- `Drawing::add_path(d: &str, attrs)` — the writer; one `add_shape` with
  `d` in the number format and `data-ink` set. The profile already reads,
  hit-tests (through `path::flatten`), moves and resizes a `<path>`.
- A stroke model that takes points (and pressure, when there is any) as
  the pointer moves and produces the `d`: the core's nib, or a seam where
  the host's outline comes in. The canvas previews the stroke in flight as
  it does a create drag — the shape is added on lift, as one step.

Done when `Tool.draw.isAvailable` is true, a stroke drawn on the canvas is
one `<path data-ink>` and one undo step, and the profile's `data-ink` row
is no longer marked reserved.
