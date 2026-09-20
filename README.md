---
part_of: id:org/kv2bv2m
title: thorn
contents:
- '[The Diaryx drawing profile](/docs/profile.md)'
- '[Tasks](/docs/tasks/tasks.md)'
- '[Changelog](/docs/CHANGELOG.md)'
- '[thorn on diaryx.org](/www/index.md)'
- '[Audiences](/vocab/audiences.md)'
config: .config/prov.yaml
registry: registry.yaml
id: qf27cd5
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
> and `thorn-svg-ffi`; the binary and the Swift product are `thorn` and `Thorn`.

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
blank line still does ([docs/tasks/delete-leaves-its-line.md](docs/tasks/delete-leaves-its-line.md)).

## Layout

| part | what it is |
|------|------------|
| [`docs/profile.md`](docs/profile.md) | **What a Diaryx drawing SVG is.** The marker, `data-id` on every shape, the `data-` vocabulary, the number format that makes a re-export byte-stable. Held to by `thorn check` and the core's fixture tests. |
| [`crates/thorn-svg-core`](crates/thorn-svg-core) | **The core.** Pure Rust over `twig-doc`: the shape model read off the element tree after every edit, the profile as code, the geometry under move and resize, and the gestures. No UI, no filesystem, no rendering. |
| [`crates/thorn-svg-ffi`](crates/thorn-svg-ffi) | **The UniFFI binding.** One object, `Drawing`; the core's records mirrored as value types. A host links it into its one Rust staticlib. |
| [`packages/thorn-swift`](packages/thorn-swift) | **The Swift package.** `ThornFFI` is the committed generated binding; `Thorn` is `DrawingDocument` (the gestures with Foundation types at the edges), `CanvasModel` (the canvas with no view in it: tool, selection, the drag in flight, drawing into a `CGContext` through resvg-swift), `DrawingCanvasView` (AppKit / UIKit) and `DrawingEditor` (SwiftUI, with the toolbar). `Toolset.swift` is the toolbar's shape, Excalidraw's: lock, hand, select, rectangle, diamond, ellipse, arrow, line, draw, text, note, eraser, each with its keys (`R` or `2`, `Q` for the lock, Escape for select); the layering and grouping commands are a menu. A tool the core has no gesture for is in the toolbar, disabled, with a task naming what it needs. `Package.swift` sits at the repo root because SwiftPM needs it there. |
| [`apps/thorn-mac`](apps/thorn-mac) | **A window around `DrawingEditor`**, for seeing a change work on the Mac. Its own package, because it force-loads the staticlib with a flag a by-version consumer may not carry. |
| [`apps/thorn-svg`](apps/thorn-svg) | **The CLI.** `check` holds a file to the profile, `shapes` lists them, `render` makes a PNG through resvg — the profile testable with no screen. |

## Gestures

Each is one twig operation, so undo is twig's and the editor keeps no second
history:

| gesture | twig | status |
|---------|------|--------|
| add rect, ellipse, line, arrow, label | `insert_after` / `insert_child` | done: an arrow is a `<line>` with `data-arrow`, its head the drawing's `<style>` |
| re-word a label, wrap it | `edit_range` over the `<text>`'s interior | done: `set_text`, and `set_width` flowing the words into `<tspan>` lines at `data-width` (measured by the host's layout); the canvas opens a field on a double-click or at the click with the label tool, and a label's box handles set the width it wraps to |
| delete | `delete` | done ([its line stays](docs/tasks/delete-leaves-its-line.md)) |
| undo, redo | `undo` / `redo` | done |
| move, resize | `set_node_attrs` | done: the attributes for rect, ellipse, circle, line, polyline, polygon, text, image, the `d` for a `<path>`; a `transform` only for a `<g>` or under a rotation |
| forward, back, to front, to back | `move_before` / `move_after` | done, among sibling shapes |
| group, ungroup | `edit_range` around the members / over the `<g>`, `move_after` to bring a stray member up, folded into one undo step | done; a group's `transform` is baked into its members' attributes on ungroup |
| select, hit-test, handles | — (pure geometry) | done, through `transform` chains; a `<text>`'s box is measured by the host's layout (`measure::Usvg` in the binding — resvg's, so it is the box drawn) |
| bind an arrow | `set_node_attrs` on the `<line>`, folded into the move that made it follow | done: `data-from` / `data-to` on a `<line>`; a bound end sits on its shape's edge and follows it; dropping an endpoint handle on a shape binds it |
| freehand ink | — | reserved in the profile |

## Building

```sh
cargo xtask ci          # fmt, clippy, tests, per-crate isolation, binding drift
cargo xtask bindings    # regenerate the committed Swift binding after an FFI change
scripts/test-swift.sh   # the Swift package's tests, on a Mac
cargo run -p thorn-svg -- check crates/thorn-svg-core/tests/fixtures/boxes-and-arrow.svg
cargo xtask mac drawing.svg   # build the staticlib, open the Mac app around a drawing
```

## Linking

The Swift package builds the binding from source and expects the Rust
staticlib to be linked by the app. An app that already links a Rust FFI crate
of its own — the Diaryx app does — makes `thorn-svg-ffi` a Cargo dependency
of that crate, so the scaffolding lands in the one archive it already
force-loads; two Rust staticlibs cannot share an executable. An app with no
Rust of its own builds `crates/thorn-svg-ffi`'s staticlib and force-loads
that, as `scripts/test-swift.sh` does.

## Where it fits

Depends on `twig-doc` by crates.io version, as every cross-repo edge in the
org does; the Diaryx app will depend on it, and nothing else does. The org's
proposal, *A drawing editor over twig's SVG, as its own repository*, is the
argument for this repository's existence and its boundaries.

## License

MIT or Apache-2.0, at your option.
