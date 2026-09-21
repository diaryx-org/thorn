//! thorn-svg-ffi — thorn-svg-core across UniFFI.
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

use thorn_svg_core as core;

// Linked, not used: see the dependency's note in Cargo.toml. The `as _` is
// what makes rustc treat the crate as referenced and carry its objects into
// the staticlib.
use resvg_uniffi as _;

uniffi::setup_scaffolding!();

/// Why an open or a gesture refused; mirrors `thorn_svg_core::Error`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum DrawingError {
    #[error("{message}")]
    Parse { message: String },
    #[error("the document element is not <svg>")]
    NotSvg,
    #[error("no shape with data-id {id:?}")]
    NoSuchShape { id: String },
    #[error("{gesture} is not defined for this kind of shape")]
    Unsupported { gesture: String, kind: ShapeKind },
    #[error("the shapes to group are not siblings")]
    NotSiblings,
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
            core::Error::Unsupported { gesture, kind } => Self::Unsupported {
                gesture: gesture.to_string(),
                kind: kind.into(),
            },
            core::Error::NotSiblings => Self::NotSiblings,
            core::Error::Edit(inner) => Self::Edit {
                message: format!("{inner:?}"),
            },
        }
    }
}

/// Mirrors `thorn_svg_core::ShapeKind`.
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

/// Mirrors `thorn_svg_core::Heads`: which ends of an arrow have a head.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum Heads {
    End,
    Start,
    Both,
}

impl From<core::Heads> for Heads {
    fn from(h: core::Heads) -> Self {
        match h {
            core::Heads::End => Self::End,
            core::Heads::Start => Self::Start,
            core::Heads::Both => Self::Both,
        }
    }
}

impl From<Heads> for core::Heads {
    fn from(h: Heads) -> Self {
        match h {
            Heads::End => Self::End,
            Heads::Start => Self::Start,
            Heads::Both => Self::Both,
        }
    }
}

/// Mirrors `thorn_svg_core::Dash`: how a stroke is broken.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum Dash {
    Dashed,
    Dotted,
}

impl From<core::Dash> for Dash {
    fn from(d: core::Dash) -> Self {
        match d {
            core::Dash::Dashed => Self::Dashed,
            core::Dash::Dotted => Self::Dotted,
        }
    }
}

impl From<Dash> for core::Dash {
    fn from(d: Dash) -> Self {
        match d {
            Dash::Dashed => Self::Dashed,
            Dash::Dotted => Self::Dotted,
        }
    }
}

/// Mirrors `thorn_svg_core::Weight`: how heavy a stroke is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Weight {
    Thin,
    Bold,
}

impl From<core::Weight> for Weight {
    fn from(w: core::Weight) -> Self {
        match w {
            core::Weight::Thin => Self::Thin,
            core::Weight::Bold => Self::Bold,
        }
    }
}

impl From<Weight> for core::Weight {
    fn from(w: Weight) -> Self {
        match w {
            Weight::Thin => Self::Thin,
            Weight::Bold => Self::Bold,
        }
    }
}

/// Mirrors `thorn_svg_core::Hue`: a colour of the palette, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Hue {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    Pink,
    Grey,
}

impl From<core::Hue> for Hue {
    fn from(h: core::Hue) -> Self {
        match h {
            core::Hue::Red => Self::Red,
            core::Hue::Orange => Self::Orange,
            core::Hue::Yellow => Self::Yellow,
            core::Hue::Green => Self::Green,
            core::Hue::Blue => Self::Blue,
            core::Hue::Violet => Self::Violet,
            core::Hue::Pink => Self::Pink,
            core::Hue::Grey => Self::Grey,
        }
    }
}

impl From<Hue> for core::Hue {
    fn from(h: Hue) -> Self {
        match h {
            Hue::Red => Self::Red,
            Hue::Orange => Self::Orange,
            Hue::Yellow => Self::Yellow,
            Hue::Green => Self::Green,
            Hue::Blue => Self::Blue,
            Hue::Violet => Self::Violet,
            Hue::Pink => Self::Pink,
            Hue::Grey => Self::Grey,
        }
    }
}

/// Every hue, in the order a palette shows them.
#[uniffi::export]
pub fn hues() -> Vec<Hue> {
    core::Hue::ALL.into_iter().map(Hue::from).collect()
}

/// What the template draws a hue as — the `color` a `data-color` sets and
/// the tint a `data-fill` is — on a light page and a dark one, as CSS
/// hex; what a palette swatch shows.
#[uniffi::export]
pub fn hue_hex(hue: Hue, dark: bool, tint: bool) -> String {
    let hue: core::Hue = hue.into();
    if tint {
        hue.tint(dark)
    } else {
        hue.stroke(dark)
    }
    .to_string()
}

/// Mirrors `thorn_svg_core::Pen`: the words the next shape is added with.
#[derive(Clone, Copy, Debug, Default, uniffi::Record)]
pub struct Pen {
    pub dash: Option<Dash>,
    pub hue: Option<Hue>,
    pub fill: Option<Hue>,
    pub weight: Option<Weight>,
    /// A box's corner radius, in user units.
    pub corner: Option<f64>,
}

impl From<core::Pen> for Pen {
    fn from(p: core::Pen) -> Self {
        Self {
            dash: p.dash.map(Into::into),
            hue: p.hue.map(Into::into),
            fill: p.fill.map(Into::into),
            weight: p.weight.map(Into::into),
            corner: p.corner,
        }
    }
}

impl From<Pen> for core::Pen {
    fn from(p: Pen) -> Self {
        Self {
            dash: p.dash.map(Into::into),
            hue: p.hue.map(Into::into),
            fill: p.fill.map(Into::into),
            weight: p.weight.map(Into::into),
            corner: p.corner,
        }
    }
}

/// Mirrors `thorn_svg_core::Nib`: the rule an ink stroke's outline is
/// drawn by.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum Nib {
    Monoline,
}

impl From<Nib> for core::Nib {
    fn from(n: Nib) -> Self {
        match n {
            Nib::Monoline => Self::Monoline,
        }
    }
}

/// Mirrors `thorn_svg_core::Note`: a note's box and label, by `data-id`.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Note {
    pub frame: String,
    pub label: String,
}

impl From<core::Note> for Note {
    fn from(n: core::Note) -> Self {
        Self {
            frame: n.frame,
            label: n.label,
        }
    }
}

/// One attribute of a shape; a bare attribute has no value.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Attribute {
    pub name: String,
    pub value: Option<String>,
}

/// Mirrors `thorn_svg_core::Shape`, minus the node id, which is not stable
/// across edits and has no meaning to a host.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Shape {
    pub kind: ShapeKind,
    pub id: Option<String>,
    pub group: Option<String>,
    pub depth: u32,
    pub attrs: Vec<Attribute>,
    /// A `<text>`'s characters, whitespace collapsed; `None` otherwise.
    pub text: Option<String>,
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
            text: s.text.clone(),
        }
    }
}

/// Mirrors `thorn_svg_core::Finding`, with the rule as its one-line text.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Finding {
    pub rule: String,
    pub shape: Option<String>,
    pub message: String,
}

/// Mirrors `thorn_svg_core::Rect`.
#[derive(Clone, Copy, Debug, uniffi::Record)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Mirrors `thorn_svg_core::Bounds`.
#[derive(Clone, Copy, Debug, uniffi::Record)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<Bounds> for core::Bounds {
    fn from(b: Bounds) -> Self {
        Self {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }
    }
}

impl From<core::Bounds> for Bounds {
    fn from(b: core::Bounds) -> Self {
        Self {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }
    }
}

/// Mirrors `thorn_svg_core::Handle`.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl From<Handle> for core::Handle {
    fn from(h: Handle) -> Self {
        match h {
            Handle::TopLeft => Self::TopLeft,
            Handle::Top => Self::Top,
            Handle::TopRight => Self::TopRight,
            Handle::Right => Self::Right,
            Handle::BottomRight => Self::BottomRight,
            Handle::Bottom => Self::Bottom,
            Handle::BottomLeft => Self::BottomLeft,
            Handle::Left => Self::Left,
        }
    }
}

impl From<core::Handle> for Handle {
    fn from(h: core::Handle) -> Self {
        match h {
            core::Handle::TopLeft => Self::TopLeft,
            core::Handle::Top => Self::Top,
            core::Handle::TopRight => Self::TopRight,
            core::Handle::Right => Self::Right,
            core::Handle::BottomRight => Self::BottomRight,
            core::Handle::Bottom => Self::Bottom,
            core::Handle::BottomLeft => Self::BottomLeft,
            core::Handle::Left => Self::Left,
        }
    }
}

/// Mirrors `thorn_svg_core::measure::Font`: the face a label lays out in.
#[derive(Clone, Debug, uniffi::Record)]
pub struct Font {
    pub size: f64,
    pub family: String,
    pub post_script_name: String,
}

/// A point in user units.
#[derive(Clone, Copy, Debug, uniffi::Record)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// The bytes a new drawing is created with: `thorn_svg_core::profile::TEMPLATE`.
#[uniffi::export]
pub fn template() -> String {
    core::profile::TEMPLATE.to_string()
}

/// The margin the page keeps around the shapes, in user units:
/// `thorn_svg_core::PAGE_MARGIN`.
#[uniffi::export]
pub fn page_margin() -> f64 {
    core::PAGE_MARGIN
}

/// Whether `source` is a drawing of the profile — an `<svg>` whose root
/// carries `data-diaryx-drawing`. A host's sniff for which surface opens an
/// `.svg`; not a conformance check, which is [`Drawing::check`].
#[uniffi::export]
pub fn is_drawing(source: String) -> bool {
    core::profile::is_drawing(&source)
}

/// Where a handle sits on a box.
#[uniffi::export]
pub fn handle_position(handle: Handle, bounds: Bounds) -> Point {
    let (x, y) = core::Handle::from(handle).position(bounds.into());
    Point { x, y }
}

/// The handle of `bounds` within `tolerance` of the point, if any.
#[uniffi::export]
pub fn handle_at(bounds: Bounds, x: f64, y: f64, tolerance: f64) -> Option<Handle> {
    core::Handle::at(bounds.into(), x, y, tolerance).map(Into::into)
}

/// `bounds` after `handle` is dragged by `(dx, dy)`.
#[uniffi::export]
pub fn handle_drag(handle: Handle, bounds: Bounds, dx: f64, dy: f64) -> Bounds {
    core::Handle::from(handle)
        .drag(bounds.into(), dx, dy)
        .into()
}

/// Mirrors `thorn_svg_core::End`: an end of a `<line>` arrow.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum End {
    /// `(x1, y1)`, bound by `data-from`.
    From,
    /// `(x2, y2)`, bound by `data-to`.
    To,
}

impl From<End> for core::End {
    fn from(e: End) -> Self {
        match e {
            End::From => Self::From,
            End::To => Self::To,
        }
    }
}

/// Mirrors `thorn_svg_core::Connector`: a line-like shape's ends, and its
/// control point when bent. `midpoint` is where the bend handle sits —
/// on the stroke, half-way along.
#[derive(Clone, Copy, Debug, uniffi::Record)]
pub struct Connector {
    pub from: Point,
    pub to: Point,
    pub control: Option<Point>,
    pub midpoint: Point,
}

impl From<core::Connector> for Connector {
    fn from(c: core::Connector) -> Self {
        let p = |(x, y): (f64, f64)| Point { x, y };
        Self {
            from: p(c.from),
            to: p(c.to),
            control: c.control.map(p),
            midpoint: p(c.midpoint()),
        }
    }
}

/// Mirrors `thorn_svg_core::Order`.
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum Order {
    Forward,
    Backward,
    ToFront,
    ToBack,
}

impl From<Order> for core::Order {
    fn from(o: Order) -> Self {
        match o {
            Order::Forward => Self::Forward,
            Order::Backward => Self::Backward,
            Order::ToFront => Self::ToFront,
            Order::ToBack => Self::ToBack,
        }
    }
}

/// A drawing being edited. See `thorn_svg_core::Drawing`.
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
    /// Open an SVG's text. A `<text>`'s bounds are measured by resvg's
    /// layout over the system's fonts — the layout the canvas draws with.
    #[uniffi::constructor]
    pub fn open(source: String) -> Result<Arc<Self>, DrawingError> {
        let mut inner = core::Drawing::open(&source)?;
        inner.set_measure(Box::new(core::measure::Usvg));
        Ok(Arc::new(Self {
            inner: Mutex::new(Inner(inner)),
        }))
    }

    /// A new, empty drawing — the profile's template opened. What a host's
    /// `New Drawing` starts from; [`template`] is the same bytes for a host
    /// that writes the file before it opens it.
    #[uniffi::constructor]
    pub fn fresh() -> Arc<Self> {
        let mut inner = core::Drawing::fresh();
        inner.set_measure(Box::new(core::measure::Usvg));
        Arc::new(Self {
            inner: Mutex::new(Inner(inner)),
        })
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

    /// Add an ellipse filling `bounds`; returns its `data-id`.
    pub fn add_ellipse(&self, bounds: Bounds) -> Result<String, DrawingError> {
        Ok(self.lock().add_ellipse(bounds.into())?)
    }

    /// Add a diamond filling `bounds` — a polygon through the midpoints of
    /// its sides; returns its `data-id`.
    pub fn add_diamond(&self, bounds: Bounds) -> Result<String, DrawingError> {
        Ok(self.lock().add_diamond(bounds.into())?)
    }

    /// Add a note filling `bounds` — a group of a box and a label wrapped
    /// to it; returns the group's `data-id`.
    pub fn add_note(&self, bounds: Bounds, text: String) -> Result<String, DrawingError> {
        Ok(self.lock().add_note(bounds.into(), &text)?)
    }

    /// A note's box and label, when `id` is a note.
    pub fn note(&self, id: String) -> Option<Note> {
        self.lock().note(&id).map(Into::into)
    }

    /// Add a line; returns its `data-id`.
    pub fn add_line(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> Result<String, DrawingError> {
        Ok(self.lock().add_line(x1, y1, x2, y2)?)
    }

    /// Add an arrow — a line with `data-arrow` saying which ends have a
    /// head; returns its `data-id`.
    pub fn add_arrow(
        &self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        heads: Heads,
    ) -> Result<String, DrawingError> {
        Ok(self.lock().add_arrow(x1, y1, x2, y2, heads.into())?)
    }

    /// Which ends of a shape have a head, or `None` for a shape that is
    /// not an arrow.
    pub fn heads(&self, id: String) -> Option<Heads> {
        self.lock()
            .shape(&id)
            .and_then(|s| s.heads())
            .map(Heads::from)
    }

    /// Add a freehand stroke: the outline `nib` makes of the centreline
    /// `points` at `widths` (one for all, or one per point), with the
    /// centreline and widths beside it; returns its `data-id`.
    pub fn add_ink(
        &self,
        points: Vec<Point>,
        widths: Vec<f64>,
        nib: Nib,
    ) -> Result<String, DrawingError> {
        let pts: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
        Ok(self.lock().add_ink(&pts, &widths, nib.into())?)
    }

    /// Add a label anchored at `(x, y)`; returns its `data-id`.
    pub fn add_text(&self, x: f64, y: f64, text: String) -> Result<String, DrawingError> {
        Ok(self.lock().add_text(x, y, &text)?)
    }

    /// Replace a `<text>`'s characters; plain text, written escaped.
    pub fn set_text(&self, id: String, text: String) -> Result<(), DrawingError> {
        Ok(self.lock().set_text(&id, &text)?)
    }

    /// The size a label lays out at, in user units — its `font-size`, or
    /// what a stylesheet gave it; with `None`, a new label's.
    pub fn font_size(&self, id: Option<String>) -> f64 {
        self.lock().font_size(id.as_deref())
    }

    /// The face and size a label lays out in, as resvg resolved them —
    /// `Helvetica` for a stylesheet's `sans-serif`; with `None`, a new
    /// label's. `None` when the label lays out to nothing.
    pub fn font(&self, id: Option<String>) -> Option<Font> {
        self.lock().font(id.as_deref()).map(|f| Font {
            size: f.size,
            family: f.family,
            post_script_name: f.post_script_name,
        })
    }

    /// Wrap a label to `width` user units, its words flowed into `<tspan>`
    /// lines; or, with `None`, put them back on one line. One undo step.
    pub fn set_width(&self, id: String, width: Option<f64>) -> Result<(), DrawingError> {
        Ok(self.lock().set_width(&id, width)?)
    }

    /// Delete the shape with this `data-id`.
    pub fn delete(&self, id: String) -> Result<(), DrawingError> {
        Ok(self.lock().delete(&id)?)
    }

    /// Delete several shapes as one undo step.
    pub fn delete_all(&self, ids: Vec<String>) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().delete_all(&ids)?)
    }

    /// A shape's extent in the root's user units, through its transform
    /// chain; a `<g>`'s is its members'. `None` for an empty group or a
    /// shape missing what its kind needs.
    pub fn bounds(&self, id: String) -> Option<Bounds> {
        self.lock().bounds(&id).map(Into::into)
    }

    /// The page: the root's `viewBox` as a box in user units. Every
    /// gesture that changes what is on the page fits it around every
    /// shape, [`page_margin`] out, in that gesture's undo step; an empty
    /// drawing keeps the page it has. `None` when there is no `viewBox`,
    /// or it is not four numbers.
    pub fn page(&self) -> Option<Bounds> {
        self.lock().page().map(Into::into)
    }

    /// The box around every shape, in the root's user units — what the
    /// page is fitted to. `None` for an empty drawing.
    pub fn extent(&self) -> Option<Bounds> {
        self.lock().extent().map(Into::into)
    }

    /// The topmost shape within `tolerance` of the point, in paint order. A
    /// member of a group is returned itself; `outermost` names the group.
    pub fn hit(&self, x: f64, y: f64, tolerance: f64) -> Option<Shape> {
        self.lock().hit(x, y, tolerance).map(Shape::from)
    }

    /// The outermost group a shape is in, or the shape itself.
    pub fn outermost(&self, id: String) -> Option<Shape> {
        self.lock().outermost(&id).map(Shape::from)
    }

    /// The shapes directly inside a `<g>`, in paint order.
    pub fn members(&self, id: String) -> Vec<Shape> {
        self.lock()
            .members(&id)
            .into_iter()
            .map(Shape::from)
            .collect()
    }

    /// Move a shape by `(dx, dy)`.
    pub fn move_by(&self, id: String, dx: f64, dy: f64) -> Result<(), DrawingError> {
        Ok(self.lock().move_by(&id, dx, dy)?)
    }

    /// Move several shapes by `(dx, dy)` as one undo step.
    pub fn move_all(&self, ids: Vec<String>, dx: f64, dy: f64) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().move_all(&ids, dx, dy)?)
    }

    /// Wrap sibling shapes in a new `<g>`; returns its `data-id`. One undo
    /// step.
    pub fn group(&self, ids: Vec<String>) -> Result<String, DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().group(&ids)?)
    }

    /// Replace a `<g>` with its members, its transform pushed down onto
    /// them; returns their ids. One undo step.
    pub fn ungroup(&self, id: String) -> Result<Vec<String>, DrawingError> {
        Ok(self.lock().ungroup(&id)?)
    }

    /// Where an end of a connector is, in the root's user units; `None`
    /// for what is not one.
    pub fn end_point(&self, id: String, end: End) -> Option<Point> {
        self.lock()
            .end_point(&id, end.into())
            .map(|(x, y)| Point { x, y })
    }

    /// A shape as a connector — a `<line>`, or a `<path>` of one straight
    /// or bent segment — in the root's user units; `None` for what is not
    /// one.
    pub fn connector(&self, id: String) -> Option<Connector> {
        self.lock().connector(&id).map(Into::into)
    }

    /// Bend a connector to pass through a point half-way along — a
    /// `<line>` becomes a `<path>` — and re-settle its bound ends. One
    /// undo step.
    pub fn bend(&self, id: String, x: f64, y: f64) -> Result<bool, DrawingError> {
        Ok(self.lock().bend(&id, Some((x, y)))?)
    }

    /// Straighten a bent connector: a `<line>` again. One undo step;
    /// `false` when it was a `<line>` already, and nothing was written.
    pub fn straighten(&self, id: String) -> Result<bool, DrawingError> {
        Ok(self.lock().bend(&id, None)?)
    }

    /// Bind an end of a connector to a shape, the end put on its edge; or,
    /// with no target, unbind it. One undo step.
    pub fn bind(&self, id: String, end: End, target: Option<String>) -> Result<(), DrawingError> {
        Ok(self.lock().bind(&id, end.into(), target.as_deref())?)
    }

    /// The words the next shape is added with.
    pub fn pen(&self) -> Pen {
        self.lock().pen().into()
    }

    /// Set the words the next shape is added with; not an edit.
    pub fn set_pen(&self, pen: Pen) {
        self.lock().set_pen(pen.into());
    }

    /// Whether a dash means anything on a shape: a stroked one, or a group
    /// with one inside.
    pub fn takes_dash(&self, id: String) -> bool {
        self.lock().takes_dash(&id)
    }

    /// Whether a colour means anything on a shape: anything but an image,
    /// or a group of only images.
    pub fn takes_color(&self, id: String) -> bool {
        self.lock().takes_color(&id)
    }

    /// Whether a fill means anything on a shape: a closed one, or a group
    /// with one inside.
    pub fn takes_fill(&self, id: String) -> bool {
        self.lock().takes_fill(&id)
    }

    /// How a shape's stroke is broken — a group's, what its stroked
    /// members agree on — or `None` for solid.
    pub fn dash(&self, id: String) -> Option<Dash> {
        self.lock().dash(&id).map(Dash::from)
    }

    /// A shape's hue — a group's, what its members agree on — or `None`
    /// for the drawing's ink.
    pub fn color(&self, id: String) -> Option<Hue> {
        self.lock().color(&id).map(Hue::from)
    }

    /// A shape's background hue — a group's, what its closed members
    /// agree on — or `None` for none.
    pub fn fill(&self, id: String) -> Option<Hue> {
        self.lock().fill(&id).map(Hue::from)
    }

    /// Colour a shape — every member, for a group — with a hue of the
    /// palette, or with `None` the drawing's ink. One undo step;
    /// `Unsupported` for what takes no colour.
    pub fn set_color(&self, id: String, hue: Option<Hue>) -> Result<(), DrawingError> {
        Ok(self.lock().set_color(&id, hue.map(Into::into))?)
    }

    /// `set_color` over a selection as one undo step, what takes no colour
    /// left as it is.
    pub fn set_color_all(&self, ids: Vec<String>, hue: Option<Hue>) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().set_color_all(&ids, hue.map(Into::into))?)
    }

    /// Give a closed shape a background, or with `None` none. One undo
    /// step; `Unsupported` for what is not closed.
    pub fn set_fill(&self, id: String, hue: Option<Hue>) -> Result<(), DrawingError> {
        Ok(self.lock().set_fill(&id, hue.map(Into::into))?)
    }

    /// `set_fill` over a selection as one undo step, what is not closed
    /// left as it is.
    pub fn set_fill_all(&self, ids: Vec<String>, hue: Option<Hue>) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().set_fill_all(&ids, hue.map(Into::into))?)
    }

    /// Whether a weight means anything on a shape: as `takes_dash`.
    pub fn takes_weight(&self, id: String) -> bool {
        self.lock().takes_weight(&id)
    }

    /// How heavy a shape's stroke is — a group's, what its stroked members
    /// agree on — or `None` for the template's width.
    pub fn weight(&self, id: String) -> Option<Weight> {
        self.lock().weight(&id).map(Weight::from)
    }

    /// Say how heavy a stroked shape's stroke is, or with `None` the
    /// template's width. One undo step; `Unsupported` for what is not
    /// stroked.
    pub fn set_weight(&self, id: String, weight: Option<Weight>) -> Result<(), DrawingError> {
        Ok(self.lock().set_weight(&id, weight.map(Into::into))?)
    }

    /// `set_weight` over a selection as one undo step, what is not stroked
    /// left as it is.
    pub fn set_weight_all(
        &self,
        ids: Vec<String>,
        weight: Option<Weight>,
    ) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().set_weight_all(&ids, weight.map(Into::into))?)
    }

    /// Whether a corner radius means anything on a shape: a box, or a
    /// group with one inside.
    pub fn takes_corner(&self, id: String) -> bool {
        self.lock().takes_corner(&id)
    }

    /// A box's corner radius — a group's, what its boxes agree on — or
    /// `None` for square corners.
    pub fn corner(&self, id: String) -> Option<f64> {
        self.lock().corner(&id)
    }

    /// Round a box's corners to `radius`, or with `None` square them. One
    /// undo step; `Unsupported` for what is not a box.
    pub fn set_corner(&self, id: String, radius: Option<f64>) -> Result<(), DrawingError> {
        Ok(self.lock().set_corner(&id, radius)?)
    }

    /// `set_corner` over a selection as one undo step, what is not a box
    /// left as it is.
    pub fn set_corner_all(
        &self,
        ids: Vec<String>,
        radius: Option<f64>,
    ) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().set_corner_all(&ids, radius)?)
    }

    /// The rules the drawing keeps for a darker page — the body of its
    /// `@media (prefers-color-scheme: dark)` blocks — for a canvas in dark
    /// mode to append to what resvg parses. Empty when it has none.
    pub fn dark_rules(&self) -> String {
        self.lock().dark_rules()
    }

    /// Say how a stroked shape's stroke is broken, or with `None` solid.
    /// One undo step; `Unsupported` for what is not stroked.
    pub fn set_dash(&self, id: String, dash: Option<Dash>) -> Result<(), DrawingError> {
        Ok(self.lock().set_dash(&id, dash.map(Into::into))?)
    }

    /// `set_dash` over a selection as one undo step, what is not stroked
    /// left as it is.
    pub fn set_dash_all(&self, ids: Vec<String>, dash: Option<Dash>) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().set_dash_all(&ids, dash.map(Into::into))?)
    }

    /// Say which ends of a connector have a head, or with `None` none — a
    /// plain line. One undo step; `Unsupported` for what is not a connector.
    pub fn set_heads(&self, id: String, heads: Option<Heads>) -> Result<(), DrawingError> {
        Ok(self.lock().set_heads(&id, heads.map(Into::into))?)
    }

    /// `set_heads` over a selection as one undo step, what is not a
    /// connector left as it is.
    pub fn set_heads_all(
        &self,
        ids: Vec<String>,
        heads: Option<Heads>,
    ) -> Result<(), DrawingError> {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        Ok(self.lock().set_heads_all(&ids, heads.map(Into::into))?)
    }

    /// Drop an end of a connector at a point: it goes there, bound to the
    /// topmost shape within `tolerance` — any but the arrow — or unbound.
    /// Returns what it was bound to. One undo step.
    pub fn drop_end(
        &self,
        id: String,
        end: End,
        x: f64,
        y: f64,
        tolerance: f64,
    ) -> Result<Option<String>, DrawingError> {
        Ok(self.lock().drop_end(&id, end.into(), x, y, tolerance)?)
    }

    /// Fit a shape to `to`.
    pub fn resize(&self, id: String, to: Bounds) -> Result<(), DrawingError> {
        Ok(self.lock().resize(&id, to.into())?)
    }

    /// Change a shape's place in paint order; `false` when it was already
    /// there.
    pub fn reorder(&self, id: String, order: Order) -> Result<bool, DrawingError> {
        Ok(self.lock().reorder(&id, order.into())?)
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
