---
title: A note gesture in the core
description: 'A box with a label in it, as one group: the note tool, and the model that keeps the label centred and the box around it'
author: adammharris
status: done
created: 2026-09-20
updated: 2026-09-20
part_of: '[Tasks](tasks.md)'
---
# A note gesture in the core

**Done 2026-09-20.** `Drawing::add_note(Bounds, text)` writes one
`<g data-role="note">` of a `<rect>` and a `<text>` wrapped to the box's
inner width (`NOTE_PAD` = 8 in from each side), the words flowed in the
same step; `Drawing::note(id)` names the members; `resize` on a note
resizes the box and moves and re-wraps the label as one step; move and
delete are the group's. `data-role` is the profile's term, closed to
`note` (`Rule::Role`), and a note without both members is a finding. The
note tool (`N` / `9`) drags one out and opens its label at once; a
double-click anywhere on a note edits the label; a label emptied takes
the note with it. The label sits at the top-left inside the padding —
centring it is a stylesheet's `text-anchor` or a later gesture, not this
one. `diamond-and-note.svg` is the fixture.

The toolbar has a note (`N` / `9`, `Tool.note`) and it is disabled. A note
is Excalidraw's box with text inside — a `<g>` of a `<rect>` and a
`<text>` wrapped at the box's width — and every element of it exists in
the profile: `add_rect`, `add_text`, `set_width`, `group`. What is
missing is the gesture that makes them one thing and keeps them one:

- `Drawing::add_note(Bounds, text)`: the rect, the label wrapped to its
  inner width (`data-width`), the two grouped, as **one** undo step — so
  it is one splice, not three grouped after the fact.
- Reading a note back: `Shape::kind` says group; the tool needs to know
  the group is a note so a double-click edits the label, a resize of the
  group resizes the rect and re-wraps the label, and the label stays
  centred. Whether that is a `data-` term on the `<g>` (the profile's
  vocabulary gains it) or a shape read off the group's members is the
  design question.

Done when `Tool.note.isAvailable` is true, a note dragged out on the canvas
opens its field, and resizing it keeps the text inside the box.
