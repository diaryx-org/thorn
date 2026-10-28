---
part_of: id:dx4hxn744/kv2bv2m
title: thorn
contents:
- '[The Diaryx drawing profile](/docs/profile.md)'
- '[Tasks](/docs/tasks/tasks.md)'
- '[Proposals](/docs/proposals/proposals.md)'
- '[Changelog](/docs/CHANGELOG.md)'
- '[thorn on diaryx.org](/www/index.md)'
- '[Audiences](/vocab/audiences.md)'
- '[Archive](/archive/README.md)'
config: .config/prov.yaml
registry: registry.yaml
id: qf27cd5
slug: thorn
---
# thorn

A drawing editor over [twig](https://github.com/diaryx-org/twig)'s SVG: the
kind of drawing Excalidraw and tldraw make — boxes, arrows, labels, freehand
ink — where **the file is an SVG** that opens in a browser, a file manager's
preview, a git forge and `grep`, and the editor edits it losslessly. A shape
model, a profile that says what a Diaryx drawing SVG is, and gestures that
each end in one twig splice and one undo step.

> **thorn**: a protrusion from a twig, and a sharp thing to draw with. `thorn`
> itself is taken on crates.io, so the crates are `thorn-svg`, `thorn-svg-core`
> and `thorn-svg-ffi`; the binaries are `thorn` (the CLI) and `Thorn` (the app).

```rust
use thorn_svg_core::{Drawing, Order, Rect};

let mut drawing = Drawing::open(&std::fs::read_to_string("diagram.svg")?)?;
let id = drawing.add_rect(Rect { x: 10.0, y: 10.0, width: 80.0, height: 40.0 })?;
drawing.move_by(&id, 5.0, 0.0)?;       // one set_node_attrs: x="15", nothing else touched
drawing.reorder(&id, Order::ToBack)?;  // one move_before, the whitespace kept
drawing.delete(&id)?;
drawing.undo()?;                       // twig's undo: one step, the same bytes
assert!(drawing.check().is_empty());   // holds to the profile
std::fs::write("diagram.svg", drawing.source())?;
```

## Why a tree editor

A drawing editor over an SVG is a **tree editor**, not a caret editor. The
user drags a `<rect>` and the edit is its `x` and `y`; brings it forward, and
the edit is its place among its siblings; deletes a stroke, and a `<path>` is
gone. The org has a precedent for this split: `flower` is the structural
editor over `fig`'s lossless AST, beside the text editors that edit
characters. This is the same thing over twig's SVG, and the bytes it did not
touch — comments, indentation, a `<style>` block, an attribute it does not
know — survive exactly.

The line held throughout: **how SVG is spelled is twig's; what a rectangle is
is ours.** twig learns nothing from this repository, ever. Where twig lacks a
gesture the editor waits rather than re-deriving format knowledge over the
raw splice — move, resize and reorder waited on twig 3.8.1's
`set_node_attrs` and `move_before`/`move_after`, and a deleted shape's
blank line still does ([docs/tasks/closed/delete-leaves-its-line.md](/docs/tasks/closed/delete-leaves-its-line.md)).

## Layout

| part | what it is |
|------|------------|
| [`docs/profile.md`](docs/profile.md) | **What a Diaryx drawing SVG is.** The marker, `data-id` on every shape, the `data-` vocabulary, the number format that makes a re-export byte-stable. Held to by `thorn check` and the core's fixture tests. |
| [`crates/thorn-svg-core`](crates/thorn-svg-core) | **The core.** Pure Rust over `twig-doc`: the shape model read off the element tree after every edit, the profile as code, the geometry under move and resize, and the gestures. No UI, no filesystem, no rendering. |
| [`crates/thorn-svg-ffi`](crates/thorn-svg-ffi) | **The UniFFI binding.** One object, `Drawing`; the core's records mirrored as value types. The Diaryx app calls it as a Rust crate; its Swift binding went with the Swift frontend into the archive. |
| [`crates/thorn-dioxus`](crates/thorn-dioxus) | **The editor, as Dioxus components.** `Canvas` is the canvas with no view in it: tool, selection, the pen — the words the next shape is born with — the drag in flight and its preview, undone before the next so a gesture lands as one undo step, the zoom and pan — a scroll's, a pinch's, ⌘+/⌘−/⌘0's — and what is drawn over the picture. `DrawingCanvas` draws it: the page as a sheet on a desk in the light or dark appearance, the drawing's own SVG inline over the page and every shape off its edge, so a shape dragged off the page is seen taking it along, the file's dark-page rules applied for a dark one, the selection and its handles, and the field a label is typed into. `DrawingEditor` floats the toolbar over it in [unit](https://github.com/diaryx-org/unit)'s controls, Excalidraw's: lock, hand, select, rectangle, diamond, ellipse, arrow, line, draw, text, note, eraser, each with its keys (`R` or `2`, `Q` for the lock, Escape for select), the layering and grouping commands a menu; under the tools, an options strip — the palette and a background, then weight, dash, corners and arrowheads — for the selection, or for the tool when nothing is selected; the zoom at the bottom right. No renderer: the host says how large the canvas is and hands it the keys. |
| [`apps/thorn-dioxus`](apps/thorn-dioxus) | **Thorn**, the drawing app, in Rust from `main` up: the editor in a window drawn by Dioxus's native renderer (Blitz on wgpu, no webview), as Leaf is. Opens, edits and saves `.svg` — New, Open, Save and Save As in its bar and on ⌘N, ⌘O, ⌘S and ⇧⌘S, through the system's dialogs. What the Swift app did that it does not yet is [a task](docs/tasks/what-the-dioxus-app-does-not-do-yet.md). |
| [`apps/thorn-svg`](apps/thorn-svg) | **The CLI.** `check` holds a file to the profile, `shapes` lists them, `render` makes a PNG through resvg — the profile testable with no screen. |

## Gestures

Each is one twig operation, so undo is twig's and the editor keeps no second
history:

| gesture | twig | status |
|---------|------|--------|
| add rect, diamond, ellipse, line, arrow, label, note | `insert_after` / `insert_child` | done: an arrow is a `<line>` with `data-arrow`, its head the drawing's `<style>`; a diamond a `<polygon>`; a note a `<g data-role="note">` of a box and a label wrapped to it, made and resized as one |
| freehand ink | `insert_after` | done: a monoline stroke, its outline the `<path>`'s `d` and its centreline and width beside it (`ink.rs` is the nib) |
| re-word a label, wrap it | `edit_range` over the `<text>`'s interior | done: `set_text`, and `set_width` flowing the words into `<tspan>` lines at `data-width` (measured by the host's layout); the canvas opens a field on a double-click or at the click with the label tool, and a label's box handles set the width it wraps to |
| delete | `delete` | done ([its line stays](/docs/tasks/closed/delete-leaves-its-line.md)) |
| the page follows the shapes | `set_node_attrs` on `<svg>`, folded into the gesture | done: every gesture above ends by fitting `viewBox` (and a plain-number `width`/`height`) around every shape, `PAGE_MARGIN` out, in its own undo step; `Drawing::page` reads it, `Drawing::extent` is what it follows; an empty page keeps its size ([profile rule 2](docs/profile.md#rules)) |
| undo, redo | `undo` / `redo` | done |
| move, resize | `set_node_attrs` | done: the attributes for rect, ellipse, circle, line, polyline, polygon, text, image, the `d` for a `<path>`; a `transform` only for a `<g>` or under a rotation |
| forward, back, to front, to back | `move_before` / `move_after` | done, among sibling shapes |
| group, ungroup | `edit_range` around the members / over the `<g>`, `move_after` to bring a stray member up, folded into one undo step | done; a group's `transform` is baked into its members' attributes on ungroup |
| select, hit-test, handles | — (pure geometry) | done, through `transform` chains; a `<text>`'s box is measured by the host's layout (`measure::Usvg` in the binding — resvg's, so it is the box drawn) |
| bind an arrow | `set_node_attrs` on the connector, folded into the move that made it follow | done: `data-from` / `data-to` on a connector; a bound end sits on its shape's edge and follows it; dropping an endpoint handle on a shape binds it |
| which ends have a head | `set_node_attrs` on the connector | done: `set_heads` writes `data-arrow` — `end`, `start`, `both` — or takes it off; over a selection it is one undo step, the shapes that are not connectors left alone; the editor's options strip offers it for the selected connectors, or for the arrow tool's next arrow (`docs/proposals/shape-style.md` is the rest of the strip) |
| dashed, dotted, solid | `set_node_attrs` on the shape — and `edit_range` over the `<style>`, folded in, when it has no rule for the word | done: `set_dash` writes `data-dash` on a stroked shape (a note's frame) or takes it off; `set_dash_all` over a selection is one step, the rest left alone; a `Pen` set on the drawing (`set_pen`) is the words the next shape is born with, in the splice that makes it, so the strip's choice with nothing selected costs no second step |
| sharp or round corners | `set_node_attrs` on the `<rect>` | done: `set_corner` writes `rx` — on a group's boxes, a note's frame — or takes it (and a `ry`) off; a resize keeps the radius now, as it keeps a stroke's width; the `Pen` carries it |
| thin, regular, bold | `set_node_attrs` on the shape, the template's rules folded in where a stylesheet lacks them | done: `set_weight` writes `data-weight` on a stroked shape — a group's stroked members — or takes it off; the `Pen` carries it |
| a hue, a background | `set_node_attrs` on the shape — on every member, for a group — and the palette's rules and markers into the `<style>` and `<defs>` that lack them, folded in | done: `set_color` writes `data-color`, `set_fill` writes `data-fill` on a closed shape, each a name of the palette the stylesheet gives a light and a dark hex to; `color`/`fill` read a group's as what its members agree on; `dark_rules` hoists the file's dark-page rules for a canvas to draw by; the `Pen` carries both. On a shape whose own `style` outranks the word, or in a drawing with no `<style>` to draw it by, the colour, background, dash and weight gestures edit that `style` instead (docs/profile.md, *A shape that says its own look*) |
| bend a line or an arrow | `edit_range` over the element: a `<line>` rewritten as a `<path>` with a `Q`, and back | done: the round handle at a connector's midpoint drags the curve through the point, as Excalidraw's does; a bound end re-settles to leave its shape along the tangent; dragged back onto the chord it is a `<line>` again (`docs/profile.md`, *A connector*) |
| freehand ink | — | reserved in the profile |

## Building

```sh
ship ci                 # fmt, clippy, tests, per-crate isolation
cargo run -p thorn-dioxus-app -- drawing.svg   # Thorn, on a drawing (a new one without)
cargo run -p thorn-svg -- check crates/thorn-svg-core/tests/fixtures/boxes-and-arrow.svg
cargo test -p thorn-dioxus                    # the canvas's gestures, with no window
```

Every crate and app is a member of one workspace, on Dioxus 0.8's alpha;
the root `Cargo.toml` patches in [the Blitz fork](https://github.com/diaryx-org/blitz)
(branch `leaf`) for all of them, as leaf and Diaryx do. On Linux Blitz needs
fontconfig's headers (`libfontconfig1-dev`), and the Open and Save dialogs
are the XDG desktop portal's.

The Swift frontend — the `Thorn` Swift package, the Mac and iOS app over it,
and the tasks that built, signed and released it — is
[archived](archive/README.md); `v0.2.0` is the last release that built it.

## Where it fits

Depends on `twig-doc` and `twig-xml` (SVG) by crates.io version; the binding
on `resvg-uniffi` from git, and the editor on Dioxus and unit.
The crates are not on crates.io (0.2.0 was the last upload): the Diaryx app,
the one consumer, names `thorn-svg-ffi` from git (and will name
`thorn-dioxus` for its drawing view), `{ git =
"https://github.com/diaryx-org/thorn", branch = "main" }`, and locks the
commit it builds. The org's
proposal, *A drawing editor over twig's SVG, as its own repository*, is the
argument for this repository's existence and its boundaries.

## License

MIT or Apache-2.0, at your option.
