---
title: A note gesture in the core
description: 'A box with a label in it, as one group: the note tool, and the model that keeps the label centred and the box around it'
author: adammharris
status: open
created: 2026-09-20
updated: 2026-09-20
part_of: '[Tasks](tasks.md)'
---
# A note gesture in the core

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
