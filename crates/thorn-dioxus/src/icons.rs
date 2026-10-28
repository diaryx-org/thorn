//! The icons the toolbar and the options strip show, on the 24-unit grid
//! [`unit::Icon`] strokes, beside the ones unit has (`unit::icon::paths`).
//!
//! Most are [Lucide](https://lucide.dev)'s (ISC licence, in
//! `LICENSE-LUCIDE` at the repository root), each written as one path as
//! unit writes them; the dash, arrowhead and corner glyphs are thorn's own.

/// Lucide's `hand`.
pub const HAND: &str = "M18 11V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2M14 10V4a2 2 0 0 0-2-2a2 2 0 0 0-2 2v2M10 10.5V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2v8M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15";
/// Lucide's `mouse-pointer-2`.
pub const POINTER: &str = "M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z";
/// Lucide's `square`.
pub const SQUARE: &str = "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z";
/// Lucide's `diamond`.
pub const DIAMOND: &str = "M2.7 10.3a2.41 2.41 0 0 0 0 3.41l7.59 7.59a2.41 2.41 0 0 0 3.41 0l7.59-7.59a2.41 2.41 0 0 0 0-3.41l-7.59-7.59a2.41 2.41 0 0 0-3.41 0Z";
/// Lucide's `circle`.
pub const CIRCLE: &str = "M2 12a10 10 0 1 0 20 0a10 10 0 1 0 -20 0";
/// Lucide's `move-up-right`.
pub const ARROW: &str = "M13 5h6v6M19 5 5 19";
/// A line corner to corner.
pub const LINE: &str = "M5 19 19 5";
/// Lucide's `pencil`.
pub const PENCIL: &str = "M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497zM15 5l4 4";
/// Lucide's `type`.
pub const TYPE: &str = "M4 7V4h16v3M9 20h6M12 4v16";
/// Lucide's `sticky-note`.
pub const NOTE: &str =
    "M16 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8ZM15 3v4a2 2 0 0 0 2 2h4";
/// Lucide's `eraser`.
pub const ERASER: &str = "M21 21H8a2 2 0 0 1-1.42-.587l-3.994-3.999a2 2 0 0 1 0-2.828l10-10a2 2 0 0 1 2.829 0l5.999 6a2 2 0 0 1 0 2.828L12.834 21M5.082 11.09l8.828 8.828";
/// Lucide's `lock`.
pub const LOCK: &str = "M5 11h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2ZM7 11V7a5 5 0 0 1 10 0v4";
/// Lucide's `lock-open`.
pub const UNLOCK: &str = "M5 11h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2ZM7 11V7a5 5 0 0 1 9.9-1";
/// Lucide's `layers`.
pub const LAYERS: &str = "M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83zM2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17";
/// Lucide's `redo-2`, the mirror of unit's undo.
pub const REDO: &str = "M15 14l5-5-5-5M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13";
/// Lucide's `save`.
pub const SAVE: &str = "M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7M7 3v4a1 1 0 0 0 1 1h7";
/// Lucide's `folder-open`.
pub const OPEN: &str = "M6 14l1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2";
/// Lucide's `file-plus`.
pub const NEW: &str = "M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2zM14 2v5a1 1 0 0 0 1 1h5M9 15h6M12 18v-6";

/// A solid stroke.
pub const SOLID: &str = "M3 12h18";
/// A dashed one.
pub const DASHED: &str = "M3 12h4M10 12h4M17 12h4";
/// A dotted one.
pub const DOTTED: &str = "M4 12h.01M8 12h.01M12 12h.01M16 12h.01M20 12h.01";
/// A head at the end.
pub const HEAD_END: &str = "M4 12h16M14 6l6 6-6 6";
/// A head at the start.
pub const HEAD_START: &str = "M20 12H4M10 6l-6 6 6 6";
/// A head at each end.
pub const HEAD_BOTH: &str = "M3 12h18M8 7l-5 5 5 5M16 7l5 5-5 5";
/// Square corners.
pub const SHARP: &str = "M4 4h16v16H4z";
/// Round ones.
pub const ROUND: &str = "M9 4h6a5 5 0 0 1 5 5v6a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5V9a5 5 0 0 1 5-5z";
