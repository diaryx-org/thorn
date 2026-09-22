---
title: An arrow gesture in the core
description: A `<line>` with a marker at its head, so the arrow tool draws an arrow and not a bare line; what the marker is depends on the style template
author: adammharris
status: done
created: 2026-09-20
updated: 2026-09-20
part_of: '[Closed tasks](/docs/tasks/closed/closed.md)'
---
# An arrow gesture in the core

**Done 2026-09-20**, the second way below: `Drawing::add_arrow` writes
`data-arrow` (`end`, `start`, `both`) on a `<line>`, the profile's rule 6
holds the value to those three, `Shape::heads` reads it — or a
`marker-start`/`marker-end` a file spells itself — and the template's
`<style>` draws the head. The Mac app's fresh page carries the rule and
the `<marker>`; what `New Drawing` in the app writes is still
[style-template.md](/docs/tasks/closed/style-template.md)'s.

The toolbar has an arrow (`A` / `5`, `Tool.arrow`) and it is disabled. The
profile knows an arrow as a `<line>` that may carry `data-from`/`data-to`,
and `Drawing::add_line` writes the line — but nothing writes the head, and
a line with no head is the line tool.

What the head is, is the open part. SVG's way is `marker-end="url(#…)"`
naming a `<marker>` under `<defs>`, and the profile says twice that the
marker's look belongs to the template a new drawing is created with
([style-template.md](/docs/tasks/closed/style-template.md)). So the gesture is likely:

- `Drawing::add_arrow(x1, y1, x2, y2)`: `add_line` plus an attribute
  that says "arrow" — `marker-end` pointing at a marker the template
  defines, or a `data-` term the template's `<style>` renders to one
  (`line[data-arrow] { marker-end: url(#arrow) }`), so the file stays
  a diagram in a viewer with no theme.
- Which of the two, and the marker's `<defs>` when the file has none,
  are decided together with the template; the profile's `data-`
  vocabulary gains the term, if there is one.

Done when `Tool.arrow.isAvailable` is true, an arrow dragged out renders
with a head in `thorn render` and on the canvas, and its ends bind the way
a line's do.
