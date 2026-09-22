---
title: Deleting a shape leaves its indentation line behind
description: twig's exact delete removes the element's span and nothing else, so a one-per-line file keeps a blank indented line; delete_smart does not tidy XML
author: adammharris
status: done
created: 2026-09-19
updated: 2026-09-22
part_of: '[Closed tasks](/docs/tasks/closed/closed.md)'
---
# Deleting a shape leaves its indentation line behind

`Drawing::delete` calls twig's `delete`, which removes exactly the
element's span. In a file that keeps one shape per line — which is what the
writer emits — that leaves the line's indentation behind:

```
  <!-- keep me -->
  ⎵⎵
  <g data-id="s2">…
```

twig's `delete_smart` tidies surrounding blank lines for a block node in
the prose formats and does the same thing as `delete` here. What a deleted
element does to the whitespace around it is a fact about how XML is spelled,
so the fix is twig's — the same shape as `moveNode` "carrying its
indentation" on the drawing branch — and not a whitespace heuristic here.

Done when twig's XML delete (or a gesture on its drawing branch) takes the
element's line with it, this crate pins that release, and the test
`delete_and_undo_are_one_step_each_and_touch_nothing_else` asserts the
tidy form.

**Progress.** twig's `delete_smart` now takes an element alone on an
indented line with its indentation and newline (twig `b03ed3e2`, unreleased).
`Drawing::delete_all` moves from `delete` to `delete_smart` and the test
asserts the tidy form; that change passes `cargo xtask ci` against the twig
checkout and lands with the `twig-doc` pin once twig releases.

**Resolved.** twig 3.9.2 carries the fix; this crate requires
`twig-doc = "3.9.2"` and `Drawing::delete_all` calls `delete_smart`, in the
commit that closes this task.
