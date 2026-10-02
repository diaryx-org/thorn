---
title: Style a shape the editor did not write, through css.rs
description: The colour, dash and weight words do nothing on a shape styled inline; the gestures edit its `style` through fig instead
author: adammharris
status: done
created: 2026-10-01
updated: 2026-10-01
part_of: '[Tasks](tasks.md)'
---
# Style a shape the editor did not write, through css.rs

An SVG from Inkscape styles every shape inline —
`style="fill:none;stroke:#000000;stroke-width:0.36px"` — and an inline
declaration outranks every stylesheet rule, so `set_color`, `set_dash` and
`set_weight` write a word that changes nothing on such a shape. The editor
has to edit the declaration the shape already has.

`crates/thorn-svg-core/src/css.rs` is that edit: CSS as a fig language, a
stylesheet and a declaration list, every change a splice that keeps the
rest of the bytes (`declaration`, `with_declaration`). It needs fig 5.1,
the first release whose flow members can be joined by `;` (fig's
`docs/proposals/separated-entries.md`).

Done when:

1. ~~fig 5.1.0 is on crates.io, and `fig = "5.1"` is the requirement.~~ Done.
2. ~~A gesture that sets a colour, a dash or a weight on a shape whose
   `style` declares that property rewrites the declaration, in the
   gesture's undo step, and leaves the word off.~~ Done, for the
   background too, and in a drawing with no `<style>` to draw a word by
   (docs/profile.md, *A shape that says its own look*).
3. ~~`style.rs`'s rule-walking is fig over the `css` dialect: `rules`,
   `with_rules` and `widened_for_paths` read and edit the tree, and the
   string scanning goes.~~ Done, read rather than edited: `style.rs`
   reads every rule, selector and declaration through
   `css::stylesheet`, and its hand-rolled lexing is gone; but its edits
   put a declaration first in a block and a rule between two, and fig's
   editor inserts a member only after the last, so they are splices at
   the spans the parser gives.

Not done here, and not part of this task: a shape coloured by a class
rule (Illustrator's `.st0{fill:#446EE5}`) or by its group's `style`
keeps that colour, since the rule or the group is shared; and a shape
added with a `Pen` in a drawing with no `<style>` is born with words that
draw as nothing.
