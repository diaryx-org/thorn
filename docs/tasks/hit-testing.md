---
title: Hit-testing and selection in the core
description: What is under the pointer, which handle of the selection it grabbed, and the bounds a handle set is drawn from — pure geometry, testable with no screen
author: adammharris
status: in-progress
created: 2026-09-19
updated: 2026-09-20
part_of: '[Tasks](tasks.md)'
---
# Hit-testing and selection in the core

**In progress.** `hit::hits`, `Drawing::hit`, `Handle` (position, `at`,
`drag`) landed 2026-09-20 for every attribute-stated kind: a stroke counts
as its centreline within a tolerance, a fill as its interior, a polygon by
even-odd. What is left is below: a `<path>` and a `<g>` (their `d` and
their members), measured `<text>` bounds (the core uses a nominal box
around the anchor), and arrow bindings.

The core knows what a rectangle is; it should also know whether a point is
inside one. Pure geometry over the shape list, so the Mac, iOS and the
composer share one answer and it is tested against fixtures with no screen:

- `Drawing::bounds(id)` for a `<path>` and a `<g>` (the attribute-stated
  kinds have it) (its `d` parsed for its extent; a `<text>` needs a font metric
  the host supplies, so its bounds are an estimate the host may override).
- `Drawing::hit(point) -> Option<&Shape>`: the topmost shape whose
  silhouette contains the point, groups resolving to the group.
- `Selection` with its handle set, and `Handle::hit(point, tolerance)`.
- A bound arrow (`data-from` / `data-to`) follows its target: when the
  target moves, the arrow's endpoint is recomputed to the target's edge.

Also here, because it is the same geometry: **moving a `<path>` or a `<g>`.**
Neither has a position in its attributes, so `Drawing::move_by` reports
`Unsupported` for them. The gesture is a `transform="translate(dx dy)"` —
composed onto whatever `transform` is already there, which means parsing
the transform list — and `bounds` for a `<path>` is its `d` parsed for its
extent, with the transform applied.
