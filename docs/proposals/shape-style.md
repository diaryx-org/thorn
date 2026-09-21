---
title: How a shape says what it looks like
description: Dashes, a palette, a weight, a box's background and which ends of an arrow have a head — as words on the shape the drawing's <style> resolves, in the shape data-arrow already has
author: adammharris
status: accepted
created: 2026-09-21
updated: 2026-09-21
part_of: '[Proposals](proposals.md)'
---
# How a shape says what it looks like

## Status

**Accepted 2026-09-21.** Built one word at a time, each its own commit,
in the order under *The order of work*; this section names them as they
land.

- `data-arrow` from the editor: commit 1663707.
- `data-dash`: the commit that closes this line. One thing the argument
  below did not say: the "current style" with nothing selected is held by
  the core as a `Pen` the canvas sets, not by the canvas alone, so that a
  shape is born with its words in the one splice that makes it rather than
  restyled in a second undo step. It is still never in the file.

## The question

Excalidraw and tldraw give every shape a strip of options — stroke colour,
background, stroke style, stroke width, which ends of an arrow have a head,
whether a box's corners are round. thorn's tools have none: a drawing is
one colour, one weight, solid, and every arrow has one head at its end.
This is the argument for how those options are spelled in the file, which
is the decision everything downstream follows from — the core's gestures,
`check`, the template, and what a viewer with nothing but SVG shows.

## The precedent

The profile already has one of these, and says how it is spelled:
`data-arrow` on a connector is *what an arrow is*, and how a head looks is
the drawing's `<style>` — `line[data-arrow="end"] { marker-end:
url(#arrow) }` and the `<marker>` the template carries. The core writes the
word and never `marker-end`; a file that spells `marker-end` itself is read
as an arrow all the same.

Every option here takes that shape: a `data-` word on the shape naming what
it is, a rule in the stylesheet saying what that looks like, and the editor
writing the word and never the look.

## The vocabulary

| word | on | values | the template's rule |
|------|----|--------|---------------------|
| `data-arrow` | a connector | `end`, `start`, `both` — as now; the editor comes to write all three | as now |
| `data-dash` | a stroked shape — rect, ellipse, polygon, line, a connector's path | `dashed`, `dotted`; solid is the word's absence | `[data-dash="dashed"] { stroke-dasharray: 8 6 }`, `[data-dash="dotted"] { stroke-dasharray: 2 4; stroke-linecap: round }` |
| `data-color` | any shape | a palette name: `red`, `orange`, `yellow`, `green`, `blue`, `violet`, `pink`, `grey`; the drawing's ink is the word's absence | `[data-color="red"], #arrow-red { color: #c62828 }`, and a darker page's variant under `@media (prefers-color-scheme: dark)` |
| `data-fill` | a closed shape — rect, ellipse, polygon | the same palette names, as a tint; no background is the word's absence | `[data-fill="red"] { fill: #ffcdd2 }`, with its dark variant |
| `data-weight` | a stroked shape | `thin`, `bold`; the template's width is the word's absence | `[data-weight="thin"] { stroke-width: 1 }`, `[data-weight="bold"] { stroke-width: 4 }` |
| `rx` | a `<rect>` | a length | none — it is SVG's own attribute, already among a rect's geometry |

Not on freehand ink: a stroke's width is its `data-widths`, its outline is
filled, and a dash across a filled outline is nothing. Not on a `<text>`
beyond `data-color`: a label's size and alignment are a question about
labels, not about this. A group takes `data-color` and nothing else, and
its members inherit it as `color` cascades.

`data-color` sets `color`, not `stroke`, so one word colours everything
that is `currentColor` in the template: a box's stroke, an ink stroke's
fill, a label's glyphs, and — see below — an arrow's head.

## Why words and not values

**The cascade.** A presentation attribute (`stroke="#c62828"`) has the
lowest priority CSS gives anything, so the template's `rect { stroke:
currentColor }` beats it and the colour never shows. An attribute selector
(`[data-color="red"]`) outranks a type selector, so the word wins with no
change to the rules a drawing already has. An inline `style="…"` would win
too, but the editor would then be editing a second language inside an
attribute value, and every viewer that re-themes the file would lose.

**Dark mode.** The template names no colour on a shape; everything is
`currentColor` off the root's `color`, with a `prefers-color-scheme: dark`
rule for a browser and the editor setting `color` at render time. A palette
name keeps that: one word in the file, two hexes in the stylesheet, and a
red box is a readable red on either page. A hex on the shape is one colour
on both.

**What the file says.** `grep 'data-color="red"'` finds every red thing;
`stroke="#c62828"` finds the ones that happened to be that red. And a
stylesheet can be re-themed — a drawing pasted into a page with its own
`<style>` takes that page's palette, as an inlined copy already takes its
text colour.

What is given up: a viewer that drops the stylesheet shows the word as
nothing, where it would have shown a hex. That is already true of an
arrow's head, and the profile accepted it once; a word the stylesheet does
not know is the template's job to prevent, below.

## An arrow's head, in colour

A `<marker>`'s contents inherit from the marker's own ancestors, never from
the line that references it, so `marker path { fill: currentColor }` draws
a red arrow's head in the page's ink. SVG 2's `context-stroke` is the
answer in principle, and resvg honours it — but Quick Look, which is
Finder's preview and the space bar, does not (checked 2026-09-21: the head
stays black). So the template carries **one marker per palette colour**,
`#arrow-red` beside `#arrow`, and the colour rule names both — `[data-color="red"],
#arrow-red { color: … }` — with `line[data-arrow="end"][data-color="red"],
path[…] { marker-end: url(#arrow-red) }` choosing it. Verbose, generated,
and it draws the same in both viewers.

## A drawing from an earlier template

A file made today has no rule for `data-dash`; written onto it, the word
would be nothing. The bend set the precedent: the editor widens a
`<style>` it finds wanting, in the same undo step as the gesture that
needed it, and a stylesheet whose author has already said what the word
means is left alone. Each word here does the same — the first time a
drawing takes a `data-dash`, its stylesheet gains the template's dash rules
if it has none that select `[data-dash`; the first `data-color` brings the
colour rules and the markers into `<defs>`. That is the one place the
profile's *the editor never writes a `<style>`* bends, and it is the same
bend.

## The editor

Excalidraw's shape: a strip of options shown for the selection and, with
nothing selected, for the current tool, so a colour picked before drawing
is the next shape's. The "current style" is the canvas's and never in the
file. Applied to a selection it is one `set_node_attrs` per shape,
folded into one undo step, as a group's are; a word a shape cannot take is
skipped, not an error, so a mixed selection takes what applies.

In the core: `Drawing::set_dash`, `set_color`, `set_fill`, `set_weight`,
`set_heads` and `set_corner`, each `None` to take the word off; `Shape`
reads each back; `check` refuses a value outside the vocabulary, as it does
for `data-arrow` and `data-ink`.

## Out of scope

A colour the palette does not have. A shape that spells `stroke="#123456"`
itself is that colour in both modes, and the profile already says so; a
picker for one is a different design, because it is a value and not a
word, and it has the cascade problem above to solve. Excalidraw's
sloppiness and hachure — roughjs baking a hand-drawn look into paths —
which is the opposite of a file that reads as what it is. Opacity, and a
label's size and alignment.

## The order of work

One word per commit, each carrying its profile row, its `check`, its
template rules, its widening, the binding, and the editor's control:

1. `data-arrow` — `start` and `both` from the editor, and `set_heads`. The
   profile already says it; only the writing is new.
2. `data-dash`.
3. `data-color`, with the markers, and `data-fill` beside it since the
   palette is one thing.
4. `data-weight`.
5. `rx`.
