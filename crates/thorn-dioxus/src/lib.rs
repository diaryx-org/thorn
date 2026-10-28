//! thorn-dioxus — the drawing editor as Dioxus components, over
//! [`thorn_svg_core`].
//!
//! - [`Canvas`] — what a canvas does, with no view in it: the tool, the
//!   selection, the drag in flight and its preview, zoom and pan, and what
//!   is drawn over the picture. Testable with no window.
//! - [`DrawingCanvas`] — the canvas as a component: the page's sheet, the
//!   drawing's own SVG drawn inline, the selection over it, and the field a
//!   label is typed into. [`CanvasHandle`] is the canvas a page holds.
//! - [`DrawingEditor`] — the canvas with Excalidraw's toolbar, the options
//!   strip and the zoom over it, in [unit](https://github.com/diaryx-org/unit)'s
//!   controls.
//!
//! The components need no renderer; they are written for Blitz, which
//! cannot measure an element from a handler and moves no focus from code,
//! so the host says how large the canvas is and hands it the keys
//! ([`CanvasHandle::key`]). Each is under unit's `UnitRoot`, and the page
//! puts [`css`] in a `<style>`.

pub mod canvas;
mod editor;
pub mod icons;
mod tool;
mod view;

pub use canvas::{Appearance, Canvas, TextEdit};
pub use editor::{DrawingEditor, css};
pub use tool::Tool;
pub use view::{CanvasHandle, DrawingCanvas, use_canvas};
