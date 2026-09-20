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
6. **The vocabulary is closed.** A `data-` term the profile names takes
   only the values it lists: `data-arrow` is `end`, `start` or `both`;
   `data-ink` is `monoline`, and comes with `data-centreline` and
   `data-widths`; `data-role` is `note`, on a `<g>` with the members a
   note has. A term the profile does not name is preserved and not
   judged.

## What is a shape

An element with one of these tags, at any depth under `<svg>` except inside
`<defs>`: `rect`, `ellipse`, `circle`, `line`, `polyline`, `polygon`,
`path`, `text`, `image`, and `g`. A `<g>` is a shape (selectable, movable as
one) *and* a container of shapes; its members follow it in paint order.

A shape may carry a `transform`, and the editor honours one wherever it
finds it — in hit-testing, in the box it draws, in where a drag lands. It
prefers not to write one: a move or a resize goes into the attributes, and
a `<path>`'s goes into its `d`, rewritten as absolute `M`/`L`/`C`/`Q`/`Z`
in the profile's number format (an arc becomes cubics; the first rewrite
reshapes a hand-written `d`, and every one after is byte-stable).
Ungrouping *bakes* a group's `transform` into each member the same way —
a box's corners and sides, an ellipse's radii, a label's `font-size` —
so nothing moves and no member carries the group's matrix. What cannot be
baked keeps a `transform`: a `<g>` (nothing to bake into), any shape under
a rotation or a skew, a circle or a label under an unequal scale, a label
under a scale with no `font-size` of its own. Written, it is `translate(x
y)` when that is all it is, `matrix(a b c d e f)` otherwise, each number
in the profile's format, and none at all for the identity. `stroke-width`
is never touched: a shape scaled by a resize keeps its stroke, as it does
in every drawing app.

A `<text>`'s extent is its face's to say, and the face is the host's — the
one it draws with, or the box and the glyphs disagree. The core asks the
host (`measure::Measure`) for the label laid out where it is under
everything that styles it, once per distinct label as it stands; the
binding lends resvg's own layout over the system's fonts, so the box is
the one on screen. With no host layout the box is nominal:
`font-size` tall (12 when unset, usvg's default), six tenths of that per
character wide, placed by `text-anchor`, and as many lines of that as
`data-width` takes. A label's characters are its text nodes', every
`<tspan>` a run of its own with a space around it, whitespace collapsed —
so a wrapped label reads back as its words and re-flows on the next edit.
SVG `<text>` does not wrap on its own, and `<foreignObject>` is not
something resvg draws; `<tspan>` lines are the wrapping every viewer
shows.

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
| `data-from`, `data-to` | `<line>`, `<path>` | an arrow bound to a shape at each end: when the shape moves, the arrow's endpoint follows. The value is the shape's `data-id`. The editor binds a `<line>` (`Drawing::bind`, or dropping an endpoint handle on a shape) and keeps a bound end on its shape's edge, facing the other end — it rewrites `x1`/`y1` or `x2`/`y2` whenever the shape moves or resizes, in that gesture's undo step, and takes the binding off when the shape is deleted. A bound `<path>` is honoured as reserved: read, never rewritten. |
| `data-arrow` | `<line>` | `end`, `start` or `both`: which ends have a head. This is what an arrow *is*; how a head looks is the drawing's `<style>` — `line[data-arrow="end"], line[data-arrow="both"] { marker-end: url(#arrow) }` and the `<marker>` it names, which the template a new drawing is created with carries (docs/tasks/style-template.md). The editor writes `data-arrow` (`Drawing::add_arrow`) and never `marker-end`; a file that spells `marker-start`/`marker-end` itself is read as an arrow all the same (`Shape::heads`). Any other value is a finding. |
| `data-role` | `<g>` | `note`: a box with a label in it — a `<rect>` and a `<text>` wrapped to the box's inner width, eight units in from its edge. The editor makes one as one splice (`Drawing::add_note`), resizes it as one — the box to the new bounds, the label to its corner and re-wrapped — and moves and deletes it as any group; the label is re-worded as any label. A `<g>` with the role and without both members is a finding. |
| `data-width` | `<text>` | the width a label wraps to, in user units, in the number format. The editor flows the label's words into one `<tspan>` per line — each at the anchor's `x`, each after the first `dy="1.2em"` down — measured by the host's layout; a word longer than the width has a line to itself. Without it a label is one line. Written by a resize of the label's box; taken off by `Drawing::set_width(None)`. Any viewer draws the `<tspan>`s as they are. |
| `data-break` | `<tspan>` in a `<text>` | `hard`: this line starts where the author broke it, not where the width did. The editor writes a label with a line break of its own as `<tspan>` lines like a wrapped one's, this on the first line of each paragraph after the first, and reads the label back with a newline there. |
| `data-ink` | `<path>` | a freehand stroke, and the nib its outline was drawn by: `monoline`, a round nib at one width. The `d` is that outline, filled — right in any viewer — and the editor never writes a stroke any other way; the nibs the profile admits are exactly the ones the editor draws, so a value it cannot draw is a finding. Written by `Drawing::add_ink`. |
| `data-centreline`, `data-widths` | `<path>` with `data-ink` | the stroke as the hand made it: the centreline as a path `d` (`M x y L x y …`, or a host's cubics), and the width at each of its points — one value alone is the width everywhere, which is all a monoline has to say. The outline is a function of these and the nib, and the editor keeps it so: a move carries the centreline, a resize scales it and the widths and draws the outline again. A stroke without them is a finding, because its outline could not be drawn again. |

## What the writer emits

`thorn_svg_core::Drawing` writes an added shape as one element on its own
line, indented two spaces under `<svg>`, attributes in the order geometry,
then `data-id`, with a self-closing tag — `<rect x="10" y="10.5" width="80"
height="40" data-id="s1"/>`. A move or resize rewrites only the geometry
attributes it changes, each in its existing place among the element's
attributes, and appends one the element lacked; every other attribute keeps
its bytes. A `points` list is written as `x,y` pairs separated by one space.
A reorder moves the element and the line break and indentation ahead of it,
and nothing else. Binding an arrow appends `data-from` or `data-to` after
the attributes it has, and settling a bound end rewrites only the two
coordinates of that end, and only when they would change. It does not write a `<style>`; the app's proposal
puts one in the file it creates so a viewer with no theme shows a marker as a
marker, and that template is the app's to settle (docs/tasks/style-template.md).

## Held to by

- `thorn check <file>` — exits 1 on any finding.
- `crates/thorn-svg-core/tests/profile_fixtures.rs` — every fixture under
  `tests/fixtures/` conforms, and what the editor writes into it conforms.
- `thorn_svg_core::number` — the number format's tests are the byte-stability
  claim.
