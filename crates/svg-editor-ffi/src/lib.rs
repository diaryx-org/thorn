//! svg-editor-ffi — svg-editor-core across UniFFI.
//!
//! Swift gets one object, [`Drawing`], whose methods are the core's gestures
//! and whose queries return value types mirroring the core's. The records
//! are separate types rather than derives on the core's, so the pure crate
//! carries no FFI dependency and the wire format is versioned here, where the
//! consumer is.
//!
//! A UniFFI object is handed to Swift as a reference-counted handle whose
//! methods take `&self`, so the core's editor lives behind a [`Mutex`]. Drive
//! it from the main thread.

use std::sync::{Arc, Mutex};

use svg_editor_core as core;

uniffi::setup_scaffolding!();

/// Why an open or a gesture refused; mirrors `svg_editor_core::Error`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum DrawingError {
    #[error("{message}")]
    Parse { message: String },
    #[error("the document element is not <svg>")]
    NotSvg,
    #[error("no shape with data-id {id:?}")]
    NoSuchShape { id: String },
    #[error("{message}")]
    Edit { message: String },
}

impl From<core::Error> for DrawingError {
    fn from(e: core::Error) -> Self {
        match e {
            core::Error::Parse(inner) => Self::Parse {
                message: format!("{inner:?}"),
            },
            core::Error::NotSvg => Self::NotSvg,
            core::Error::NoSuchShape(id) => Self::NoSuchShape { id },
            core::Error::Edit(inner) => Self::Edit {
                message: format!("{inner:?}"),
            },
        }
    }
}

/// Mirrors `svg_editor_core::ShapeKind`.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Circle,
    Line,
    Polyline,
    Polygon,
    Path,
    Text,
    Group,
    Image,
}

impl From<core::ShapeKind> for ShapeKind {
    fn from(k: core::ShapeKind) -> Self {
        match k {
            core::ShapeKind::Rect => Self::Rect,
            core::ShapeKind::Ellipse => Self::Ellipse,
            core::ShapeKind::Circle => Self::Circle,
            core::ShapeKind::Line => Self::Line,
            core::ShapeKind::Polyline => Self::Polyline,
            core::ShapeKind::Polygon => Self::Polygon,
            core::ShapeKind::Path => Self::Path,
            core::ShapeKind::Text => Self::Text,
            core::ShapeKind::Group => Self::Group,
            core::ShapeKind::Image => Self::Image,
            // `ShapeKind` is non-exhaustive: a kind this binding predates is
            // still a shape the canvas can select and delete.
            _ => Self::Path,
        }
    }
}

/// One attribute of a shape; a bare attribute has no value.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Attribute {
    pub name: String,
    pub value: Option<String>,
}

/// Mirrors `svg_editor_core::Shape`, minus the node id, which is not stable
/// across edits and has no meaning to a host.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Shape {
    pub kind: ShapeKind,
    pub id: Option<String>,
    pub group: Option<String>,
    pub depth: u32,
    pub attrs: Vec<Attribute>,
}

impl From<&core::Shape> for Shape {
    fn from(s: &core::Shape) -> Self {
        Self {
            kind: s.kind.into(),
            id: s.id.clone(),
            group: s.group.clone(),
            depth: s.depth as u32,
            attrs: s
                .attrs
                .iter()
                .map(|(name, value)| Attribute {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
        }
    }
}

/// Mirrors `svg_editor_core::Finding`, with the rule as its one-line text.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Finding {
    pub rule: String,
    pub shape: Option<String>,
    pub message: String,
}

/// Mirrors `svg_editor_core::Rect`.
#[derive(Clone, Copy, Debug, uniffi::Record)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A drawing being edited. See `svg_editor_core::Drawing`.
#[derive(uniffi::Object)]
pub struct Drawing {
    inner: Mutex<Inner>,
}

struct Inner(core::Drawing);

impl std::ops::Deref for Inner {
    type Target = core::Drawing;
    fn deref(&self) -> &core::Drawing {
        &self.0
    }
}

impl std::ops::DerefMut for Inner {
    fn deref_mut(&mut self) -> &mut core::Drawing {
        &mut self.0
    }
}

// SAFETY: `core::Drawing` embeds a `twig::Editor`, which holds a
// `NonNull<TwigEditor>` and is therefore `!Send`. UniFFI hands `Drawing` to
// Swift as a reference-counted handle that must be `Send + Sync`, so `Inner`
// must be `Send`. This is sound because every access goes through
// `Drawing::lock()` — the `Mutex` serializes all reads and mutations — and
// twig's editor handle owns a plain heap allocation with no thread affinity,
// so moving the pointer between threads is fine as long as use is serialized.
// The same argument leaf-ffi makes for its `Inner`. The intended usage is
// still main-thread-driven; this permits the handle to cross threads, it does
// not invite concurrent use.
unsafe impl Send for Inner {}

#[uniffi::export]
impl Drawing {
    /// Open an SVG's text.
    #[uniffi::constructor]
    pub fn open(source: String) -> Result<Arc<Self>, DrawingError> {
        let inner = core::Drawing::open(&source)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner(inner)),
        }))
    }

    /// The current bytes — what saving writes.
    pub fn source(&self) -> String {
        self.lock().source().to_string()
    }

    /// The shapes, in paint order.
    pub fn shapes(&self) -> Vec<Shape> {
        self.lock().shapes().iter().map(Shape::from).collect()
    }

    /// Hold the drawing to the profile. Empty means it conforms.
    pub fn check(&self) -> Vec<Finding> {
        self.lock()
            .check()
            .into_iter()
            .map(|f| Finding {
                rule: f.rule.about().to_string(),
                shape: f.shape,
                message: f.message,
            })
            .collect()
    }

    /// Add a rectangle as the topmost shape; returns its `data-id`.
    pub fn add_rect(&self, rect: Rect) -> Result<String, DrawingError> {
        Ok(self.lock().add_rect(core::Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        })?)
    }

    /// Delete the shape with this `data-id`.
    pub fn delete(&self, id: String) -> Result<(), DrawingError> {
        Ok(self.lock().delete(&id)?)
    }

    /// Undo the last gesture; `false` when there was nothing to undo.
    pub fn undo(&self) -> Result<bool, DrawingError> {
        Ok(self.lock().undo()?)
    }

    /// Redo the last undone gesture; `false` when there was nothing to redo.
    pub fn redo(&self) -> Result<bool, DrawingError> {
        Ok(self.lock().redo()?)
    }
}

impl Drawing {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A poisoned lock means a gesture panicked mid-edit; twig rolled the
        // document back, so the editor is still consistent.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}
