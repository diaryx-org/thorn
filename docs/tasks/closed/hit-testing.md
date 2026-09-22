---
title: Hit-testing and selection in the core
description: What is under the pointer, which handle of the selection it grabbed, and the bounds a handle set is drawn from — pure geometry, testable with no screen
author: adammharris
status: done
created: 2026-09-19
updated: 2026-09-20
part_of: '[Closed tasks](/docs/tasks/closed/closed.md)'
---
# Hit-testing and selection in the core

**Done 2026-09-20**, over three commits. `hit::hits`, `Drawing::hit`,
`Handle` (position, `at`, `drag`) landed first, for every attribute-stated
kind: a stroke counts
as its centreline within a tolerance, a fill as its interior, a polygon by
even-odd. The same day, in the commit that added group and ungroup: a
`<path>` through its `d` flattened (`path::flatten`, over svgtypes'
simplifying parser), a `<g>` through its members (`Drawing::bounds` is
their union, `Drawing::outermost` is what a click on a member selects), and
`transform` — parsed into one matrix (`transform.rs`), composed up the
group chain, honoured by `bounds`, `hit`, `move_by` and `resize` for every
kind, and written by `move_by`/`resize` for a `<path>` or a `<g>`. Last,
in the commit that closes this: a `<text>`'s box measured by the host's
layout (`measure::Measure`; the binding lends resvg's, `measure::Usvg`,
over the system's fonts, so it is the box the canvas draws; a nominal box
from `font-size` and the character count without one), and arrow bindings
— `Drawing::bind` and `Drawing::drop_end` on a `<line>`, a bound end kept
on its shape's edge by every gesture that moves the shape, folded into
that gesture's step, and the canvas dragging a line by endpoint handles
that bind on drop. A bound `<path>` is still reserved: the profile allows
it, and the editor reads it and rewrites nothing.

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
Neither has a position in its attributes. The gesture is a
`transform="translate(dx dy)"` — composed onto whatever `transform` is
already there, which means parsing the transform list — and `bounds` for a
`<path>` is its `d` parsed for its extent, with the transform applied.
*(Done, as above — and then revised: an axis-aligned transform is baked
into the attributes instead, a path's into its `d`, so a `transform` is
written only for a `<g>` or under a rotation; `geometry::baked`.)* One limit stands: a shape under a rotation or a skew
has no axis-aligned box to fit, so `resize` reports `Unsupported` for it;
the handles are still drawn on the box around its rotated outline.
