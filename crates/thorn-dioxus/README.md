# thorn-dioxus

The thorn drawing editor as [Dioxus](https://dioxuslabs.com) components,
over `thorn-svg-core`, written for Dioxus's native renderer (Blitz) and
drawn with [unit](https://github.com/diaryx-org/unit)'s controls.

| | |
|---|---|
| `canvas.rs` | `Canvas`: the tool, the selection, the drag in flight and its preview, zoom and pan, and what is drawn over the picture. No view, so the gestures are tested with no window. |
| `view.rs` | `DrawingCanvas`, the canvas as a component, and `CanvasHandle`, the canvas a page holds, with the keys it takes. |
| `editor.rs` | `DrawingEditor`: the canvas with the toolbar, the options strip and the zoom over it. |
| `tool.rs` | The toolset, Excalidraw's: each tool's name, icon and keys, in one table. |

```rust
use dioxus::prelude::*;
use thorn_dioxus::{DrawingEditor, use_canvas};
use thorn_svg_core::Drawing;
use unit::prelude::*;

fn Page() -> Element {
    let canvas = use_canvas(Drawing::fresh);
    rsx! {
        UnitRoot {
            style { {thorn_dioxus::css()} }
            DrawingEditor { canvas, width: 800.0, height: 600.0 }
        }
    }
}
```

Blitz cannot measure an element from a handler, so the host says how large
the canvas is; and a host that reads its keys from the window (unit's
`Keys::Host`) hands them to `CanvasHandle::key`. The picture is the
drawing's own SVG put into the page as markup, so the renderer needs
dioxus-native's `html` and `svg` features; a label's field wants
`autofocus`.
