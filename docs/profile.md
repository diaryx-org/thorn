---
title: The Diaryx drawing profile
description: What a Diaryx drawing SVG is — the marker, the ids, the number format — stated once, held to by `thorn check` and the core's tests
part_of: '[thorn](/README.md)'
audience: public
---
# The Diaryx drawing profile

A Diaryx drawing is an SVG file. Not a serialization of a drawing into SVG:
the SVG *is* the document. Shapes are elements, a shape's geometry is its
attributes, z-order is sibling order, and what the editor needs beyond what
SVG says rides in `data-` attributes a viewer ignores. This page is the
profile — which elements and attributes the editor uses and what they mean —
so that "readable without the app" is a checkable promise and not a hope.

**The fallback every rule shares:** a reader that knows nothing but SVG gets
the correct silhouette in flat colour. A browser, Finder's preview, a git
forge, leaf's body and a published page all draw the same picture; only the
editor reads the `data-` vocabulary, and it degrades to nothing.

This is profile version **1**. The rules are numbered; `thorn check`
names the rule it found broken, and `thorn_svg_core::profile::Rule` is the
same list as code.

## Rules

1. **Marker.** `<svg>` carries `data-diaryx-drawing="1"` — the profile
   version. A file without it is still SVG; it is just not claiming to be a
   Diaryx drawing, and the editor's gestures still work on it.
2. **Page-shaped.** `<svg>` carries `viewBox`. The embed and the site want a
   size without laying the drawing out, and a page that grows changes its
   `viewBox` rather than having none.
3. **Every shape has an id.** Every shape element carries `data-id`, a
   non-empty string. This is how the editor, an arrow's binding and a remark
   address a shape across edits; the element's position in the tree is not
   stable, and SVG's own `id` is reserved for what SVG uses it for (`<marker
   id>`, `url(#…)`). The editor mints `s1`, `s2`, … past the largest such
   id in the file; a hand may write any string.
4. **Ids are unique.** No two shapes carry the same `data-id`.
5. **Numbers are written one way.** Every geometry attribute (`x`, `y`,
   `width`, `height`, `rx`, `ry`, `cx`, `cy`, `r`, `x1`, `y1`, `x2`, `y2`)
   is a plain decimal with at most three decimals, no trailing zeros, no
   trailing point, and `0` rather than `-0`. Three decimals is 0.006 pt of
   error on a sixty-point pen stroke, below anything a display can show;
   trimming is what keeps `10` from becoming `10.000` on a file a hand
   wrote. What this buys is **byte-stability**: an unedited re-export is the
   same bytes, so a historica diff of a drawing means the drawing changed.

## What is a shape

An element with one of these tags, at any depth under `<svg>` except inside
`<defs>`: `rect`, `ellipse`, `circle`, `line`, `polyline`, `polygon`,
`path`, `text`, `image`, and `g`. A `<g>` is a shape (selectable, movable as
one) *and* a container of shapes; its members follow it in paint order.

Everything else — `<defs>`, `<style>`, `<title>`, `<desc>`, `<metadata>`, a
comment, a processing instruction, an element the editor has never heard of
— is preserved exactly and never modelled. The editor edits through twig,
which rewrites only the span it was asked to, so the bytes the user did not
touch are the bytes that were there.

## The `data-` vocabulary

Beyond `data-id`, these are what a Diaryx drawing may carry. Each is
optional, each is ignored by any viewer, and none changes what the file
draws.

| attribute | on | means |
|-----------|----|-------|
| `data-diaryx-drawing` | `<svg>` | rule 1: the profile version |
| `data-id` | every shape | rule 3 |
| `data-from`, `data-to` | `<line>`, `<path>` | an arrow bound to a shape at each end: when the shape moves, the arrow's endpoint follows. The value is the shape's `data-id`. *Binding is not yet a gesture; the attributes are reserved.* |
| `data-ink` | `<path>` | a freehand stroke: the `d` is an outline the nib computed, filled. *Reserved: see the ink section of the org's proposal.* |
| `data-centreline`, `data-widths` | `<path>` with `data-ink` | the pen's centreline points and per-point widths the outline was computed from, so a platform with a different nib can recompute it. *Reserved.* |

## What the writer emits

`thorn_svg_core::Drawing` writes an added shape as one element on its own
line, indented two spaces under `<svg>`, attributes in the order geometry,
then `data-id`, with a self-closing tag — `<rect x="10" y="10.5" width="80"
height="40" data-id="s1"/>`. A move or resize rewrites only the geometry
attributes it changes, each in its existing place among the element's
attributes, and appends one the element lacked; every other attribute keeps
its bytes. A `points` list is written as `x,y` pairs separated by one space.
A reorder moves the element and the line break and indentation ahead of it,
and nothing else. It does not write a `<style>`; the app's proposal
puts one in the file it creates so a viewer with no theme shows a marker as a
marker, and that template is the app's to settle (docs/tasks/style-template.md).

## Held to by

- `thorn check <file>` — exits 1 on any finding.
- `crates/thorn-svg-core/tests/profile_fixtures.rs` — every fixture under
  `tests/fixtures/` conforms, and what the editor writes into it conforms.
- `thorn_svg_core::number` — the number format's tests are the byte-stability
  claim.
