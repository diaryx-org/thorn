//! svg-editor-core — a drawing editor's model over an SVG that twig edits.
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
//!   arithmetic under move and resize.

pub mod drawing;
pub mod geometry;
pub mod number;
pub mod profile;
pub mod shape;

pub use drawing::{Drawing, Error, Order, Rect};
pub use geometry::Bounds;
pub use profile::{Finding, Rule};
pub use shape::{Shape, ShapeKind};
