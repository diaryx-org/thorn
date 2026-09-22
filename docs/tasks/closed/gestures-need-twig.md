---
title: Move, resize and reorder wait on twig's drawing branch
description: The gestures that rewrite attributes or reorder siblings need setNodeAttrs and moveNode, which are on twig's unmerged branch
author: adammharris
status: done
created: 2026-09-19
updated: 2026-09-20
part_of: '[Closed tasks](/docs/tasks/closed/closed.md)'
---
# Move, resize and reorder wait on twig's drawing branch

**Done 2026-09-20.** The branch was rebased and merged, released as twig
3.8.1, and the pin moved to it. `Drawing::move_by`, `Drawing::resize` and
`Drawing::reorder` — over `set_node_attrs` and `move_before`/`move_after`,
one call and one undo step each — landed in the commit that closes this,
with `geometry.rs` under them and the fixture test
`every_gesture_is_one_step_back_to_the_same_bytes`. What is left for a
`<path>` and a `<g>`, which have no position in their attributes, is
`transform` — noted in `hit-testing.md`.

Move and resize are `setNodeAttrs`; forward, back, to-front and to-back
are `moveNode`. Both are built, on `claude/diaryx-drawing-support-4g2szs` in
`diaryx-org/twig` — five commits on 3.7.0, unmerged and unreleased — and
neither reaches `twig-doc` 3.8, which is what this workspace pins.

The core has add (`insert_child` / `insert_after`), delete, undo and redo,
which 3.8 has. It has no move, resize, or order gesture, and will not build
one over the raw splice: how an attribute rewrite or a reorder affects the
bytes around it is format knowledge, and the org has already learned once
(leaf's clipboard) that a heuristic re-deriving what twig can measure gets
deleted when twig exposes the answer.

Done when:

- twig's branch is merged and released — the version is Adam's to name —
  and the pin in `Cargo.toml` moves to it.
- `Drawing::move_by(id, dx, dy)`, `Drawing::resize(id, Rect)`, and
  `Drawing::reorder(id, Order)` exist, each one twig call and one undo
  step, with a fixture test that the bytes around the edit are untouched.
- The same three reach the FFI crate and `DrawingDocument`.
