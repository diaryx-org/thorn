//! thorn-svg-core — a drawing editor's model over an SVG that twig edits.
//!
//! A drawing editor over an SVG is a *tree* editor: the user drags a `<rect>`
//! and the edit is its `x` and `y`; brings it forward and the edit is its place
//! among its siblings; deletes a stroke and a `<path>` is gone. This crate is
//! that model. It knows what a rectangle is; [`twig`] knows how XML spells one,
//! and nothing here re-derives what twig can measure.
//!
//! Three parts:
//!
//! - [`profile`] — what a Diaryx drawing SVG is: the root marker, `data-id` on
//!   every shape, the number format, and [`profile::check`], which holds a file
//!   to it.
//! - [`Shape`] — one element the editor treats as a shape, read from twig's
//!   flat node list. Everything else in the file (`<defs>`, `<style>`, a
//!   comment) is noise the editor preserves and never models.
//! - [`Drawing`] — the editor: a `twig::Editor` over the bytes, the shape list
//!   read back after every edit, and the gestures: add, delete, move, resize,
//!   reorder, undo, redo. Each is one twig operation and therefore one undo
//!   step; the editor keeps no history of its own. [`geometry`] is the
//!   arithmetic under move and resize; [`hit`] is what is under the pointer
//!   and the handles a selection is resized by; [`measure`] is how a host
//!   lends the core its fonts, so a `<text>`'s box is the one it draws.

pub mod connector;
pub mod drawing;
pub mod geometry;
pub mod hit;
pub mod ink;
pub mod measure;
pub mod number;
pub mod path;
pub mod profile;
pub mod shape;
mod style;
pub mod transform;

pub use connector::{Connector, End};
pub use drawing::{Drawing, Error, NOTE_PAD, Note, Order, PAGE_MARGIN, Pen, Rect};
pub use geometry::Bounds;
pub use hit::Handle;
pub use ink::Nib;
pub use measure::Measure;
pub use profile::{Finding, Rule};
pub use shape::{Dash, Heads, Hue, Shape, ShapeKind, Weight};
pub use transform::Transform;
