---
title: A diamond gesture in the core
description: 'Drawing::add_polygon (and the diamond tool over it): four points at the midpoints of a box, resized as a polygon already is'
author: adammharris
status: open
created: 2026-09-20
updated: 2026-09-20
part_of: '[Tasks](tasks.md)'
---
# A diamond gesture in the core

The toolbar has a diamond (`D` / `3`, `Tool.diamond` in the Swift package)
and it is disabled: the core has no gesture that makes one. A diamond in
the profile is a `<polygon>` with four `points` at the midpoints of its
box's edges — `points="cx,y x2,cy cx,y2 x,cy"` — which the profile already
reads, hit-tests (even-odd) and resizes (`Drawing::resize` rewrites a
`points` list), so the gesture is one writer:

- `Drawing::add_polygon(points: &[(f64, f64)])`, one `add_shape` with
  `points` in the number format and the pair spelling the profile states
  (`x,y` pairs, one space between) — the general gesture, since a
  triangle or an arrowhead is the same element.
- `add_diamond(Bounds)` in the Swift `DrawingDocument`, or in the core as
  a convenience over it; the model's `.create` drag then adds one the way
  it adds a rect.

Done when `Tool.diamond.isAvailable` is true, a diamond dragged out on the
canvas is one undo step, and a fixture under `tests/fixtures/` shows what
the writer emits.
