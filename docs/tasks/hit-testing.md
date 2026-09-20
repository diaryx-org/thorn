---
title: Hit-testing and selection in the core
description: What is under the pointer, which handle of the selection it grabbed, and the bounds a handle set is drawn from — pure geometry, testable with no screen
author: adammharris
status: open
created: 2026-09-19
updated: 2026-09-19
part_of: '[Tasks](tasks.md)'
---
# Hit-testing and selection in the core

The core knows what a rectangle is; it should also know whether a point is
inside one. Pure geometry over the shape list, so the Mac, iOS and the
composer share one answer and it is tested against fixtures with no screen:

- `Drawing::bounds(id) -> Option<Bounds>` for every kind, a `<path>`
  included (its `d` parsed for its extent; a `<text>` needs a font metric
  the host supplies, so its bounds are an estimate the host may override).
- `Drawing::hit(point) -> Option<&Shape>`: the topmost shape whose
  silhouette contains the point, groups resolving to the group.
- `Selection` with its handle set, and `Handle::hit(point, tolerance)`.
- A bound arrow (`data-from` / `data-to`) follows its target: when the
  target moves, the arrow's endpoint is recomputed to the target's edge.

Depends on the move and resize gestures (`gestures-need-twig.md`) for the
half that writes back.
