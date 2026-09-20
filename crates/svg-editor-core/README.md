# svg-editor-core

The frontend-neutral core of the drawing editor: what a Diaryx drawing SVG
*is* (the [profile](../../docs/profile.md)), a shape model read off twig's
element tree, and gestures that each end in one twig splice — so undo is
twig's and the bytes the user did not touch are the bytes that were there.

```rust
use svg_editor_core::{Bounds, Drawing, Rect};

let mut drawing = Drawing::open(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" data-diaryx-drawing="1"></svg>"#)?;
let id = drawing.add_rect(Rect { x: 10.0, y: 10.0, width: 80.0, height: 40.0 })?;
drawing.move_by(&id, 5.0, 0.0)?;
drawing.resize(&id, Bounds { x: 0.0, y: 0.0, width: 50.0, height: 50.0 })?;
assert_eq!(drawing.shapes().len(), 1);
drawing.delete(&id)?;
drawing.undo()?;
assert_eq!(drawing.shapes().len(), 1);
println!("{}", drawing.source());
```

No UI, no filesystem, no rendering: bytes in, bytes out. The CLI in
`apps/svg-editor` and the UniFFI binding in `crates/svg-editor-ffi` are its
two consumers in this repository.
