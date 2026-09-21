//! The editor: a `twig::Editor` over the bytes, a shape list read back after
//! every edit, and the gestures. Each gesture is one twig operation and so
//! one undo step; undo is twig's, and the editor keeps no second history.
//!
//! Every gesture addresses its node by a *locator* computed fresh from the
//! tree — twig's locators are index paths (`"4.3"`) that count every node,
//! whitespace text included, so one is only valid until the next edit.
//!
//! A gesture that has to be several twig operations — grouping shapes that
//! are not adjacent, ungrouping a `<g>` that carries a `transform`, moving a
//! multi-selection — folds them into one undo step with twig's
//! `coalesce_last_undo`, so the promise above holds from the outside.
//!
//! Positions and boxes at this boundary are in the root's user units — what
//! the canvas measures. A shape inside a transformed `<g>`, or carrying a
//! `transform` of its own, is mapped through the chain here; [`geometry`]
//! and [`hit`](crate::hit) work in a shape's own coordinates.
//!
//! An arrow — a connector (a `<line>`, or a one-segment `<path>`, see
//! [`Connector`]) with `data-from` or `data-to` naming a shape — is kept on
//! that shape's edge: every gesture that moves a shape ends by settling the
//! arrows bound to it, in the same undo step, and deleting a shape takes
//! the bindings to it off.
//!
//! The page follows the shapes. Every gesture that changes what is on the
//! page ends by fitting the root's `viewBox` — and its `width` and `height`,
//! when they are plain numbers — around every shape, [`PAGE_MARGIN`] out
//! from their box, in the same undo step and only when it would change.
//! There is no page to set: a drawing is as big as what is drawn on it, as
//! in Excalidraw, and its `viewBox` is what a viewer with no editor shows.
//! An empty drawing keeps the page it has, since there is nothing to fit,
//! and opening a file writes nothing — a page that does not fit its shapes
//! is fitted by the first gesture.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use twig::{Editor, FlatNode, Format, Kind, NodeId};

use crate::connector::{Connector, End};
use crate::geometry::{self, Bounds, Update};
use crate::ink::{self, Nib};
use crate::measure::{Font, Measure};
use crate::number;
use crate::path::Subpath;
use crate::profile::{self, Finding};
use crate::shape::{self, Dash, Heads, Shape, ShapeKind};
use crate::style;
use crate::transform::Transform;

/// Why a gesture or an open refused.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The bytes are not XML twig can parse.
    #[error("not parseable as XML: {0:?}")]
    Parse(twig::Error),
    /// Parsed, but the document element is not `<svg>`.
    #[error("the document element is not <svg>")]
    NotSvg,
    /// No shape carries this `data-id`.
    #[error("no shape with data-id {0:?}")]
    NoSuchShape(String),
    /// The gesture is not defined for this shape: ungrouping what is not a
    /// `<g>`, resizing a shape under a rotation by a box, moving one whose
    /// `transform` maps everything to a point, binding or bending what is
    /// not a connector.
    #[error("{gesture} is not defined for this <{}>", kind.tag())]
    Unsupported {
        gesture: &'static str,
        kind: ShapeKind,
    },
    /// Grouping needs shapes with one parent: each in the same `<g>`, or
    /// each directly under `<svg>`.
    #[error("the shapes to group are not siblings")]
    NotSiblings,
    /// twig refused the edit: the target's span is not editable — a
    /// self-closed `<svg/>` has no interior to insert into — or the edit
    /// would have produced a document that no longer parses and was rolled
    /// back.
    #[error("twig refused the edit: {0:?}")]
    Edit(twig::Error),
}

/// The five characters XML reserves, as entities.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// Write attributes as markup, `skip` left out. Values are quoted as they
/// came, since twig hands them over as the bytes in the file.
fn write_attributes(out: &mut String, attrs: &[(String, Option<String>)], skip: &[&str]) {
    for (name, value) in attrs {
        if skip.contains(&name.as_str()) {
            continue;
        }
        match value {
            Some(v) if v.contains('"') => write!(out, " {name}='{v}'"),
            Some(v) => write!(out, " {name}=\"{v}\""),
            None => write!(out, " {name}"),
        }
        .expect("writing to a String");
    }
}

/// A rectangle in user units — what `add_rect` writes and what a drag over
/// a `<rect>` keeps in flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Where a reorder sends a shape among its sibling shapes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Order {
    /// One step up in paint order.
    Forward,
    /// One step down in paint order.
    Backward,
    /// Above every sibling shape.
    ToFront,
    /// Below every sibling shape — but after `<defs>`, `<style>` and
    /// whatever else is not a shape, which stay where they are.
    ToBack,
}

/// The space between a note's box and its label, in user units.
pub const NOTE_PAD: f64 = 8.0;

/// The margin the page keeps around the shapes, in user units: what a
/// gesture's fit of the `viewBox` leaves between the shapes' box and the
/// page's edge. Enough for a stroke's width, an arrowhead, and air.
pub const PAGE_MARGIN: f64 = 16.0;

/// A note's members: the `<rect>` that is its box and the `<text>` that
/// is its label, by `data-id`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    pub frame: String,
    pub label: String,
}

/// The words a shape is added with — the canvas's current options, held
/// here so an added shape is born with them in the one splice that makes
/// it, rather than restyled in a second step. Never in the file itself;
/// `Default` is no words at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pen {
    /// `data-dash` on a stroked shape.
    pub dash: Option<Dash>,
}

impl Pen {
    /// The words, as attributes, for a shape spelled by `tag` — the
    /// stroked ones take a dash, the rest nothing.
    fn words(&self, tag: &str) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let stroked = matches!(
            tag,
            "rect" | "ellipse" | "circle" | "line" | "polyline" | "polygon"
        );
        if stroked && let Some(dash) = self.dash {
            out.push(("data-dash", dash.value().to_string()));
        }
        out
    }
}

/// A drawing being edited.
pub struct Drawing {
    editor: Editor,
    /// The words the next shape is added with.
    pen: Pen,
    /// The flat tree as of the last edit; refreshed by [`Drawing::reload`].
    nodes: Vec<FlatNode>,
    root: NodeId,
    shapes: Vec<Shape>,
    source: String,
    /// The host's text layout, when it lent one.
    measure: Option<Box<dyn Measure>>,
    /// What the measurer said, by the document it was asked about.
    measured: RefCell<HashMap<String, Option<Bounds>>>,
}

impl std::fmt::Debug for Drawing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drawing")
            .field("shapes", &self.shapes.len())
            .field("bytes", &self.source.len())
            .finish()
    }
}

impl Drawing {
    /// Open an SVG's bytes. Parses as XML; refuses only what is not XML or
    /// whose document element is not `<svg>`. A file that breaks the profile
    /// still opens — [`Drawing::check`] says how — because a drawing a hand
    /// wrote is still a drawing.
    pub fn open(source: &str) -> Result<Self, Error> {
        let editor = Editor::new_str(source, Format::Svg).map_err(Error::Parse)?;
        let mut drawing = Self {
            editor,
            pen: Pen::default(),
            nodes: Vec::new(),
            root: NodeId(0),
            shapes: Vec::new(),
            source: String::new(),
            measure: None,
            measured: RefCell::new(HashMap::new()),
        };
        drawing.reload()?;
        Ok(drawing)
    }

    /// A new, empty drawing: [`profile::TEMPLATE`](crate::profile::TEMPLATE)
    /// opened. What a host's `New Drawing` starts from.
    pub fn fresh() -> Self {
        Self::open(crate::profile::TEMPLATE).expect("the template is a drawing")
    }

    /// Lend the drawing a text layout, so a `<text>`'s bounds are the box
    /// the host draws rather than the nominal one. See [`measure`](crate::measure).
    pub fn set_measure(&mut self, measure: Box<dyn Measure>) {
        self.measure = Some(measure);
        self.measured.borrow_mut().clear();
    }

    /// The words the next shape is added with.
    pub fn pen(&self) -> Pen {
        self.pen
    }

    /// Set the words the next shape is added with. Not an edit: nothing in
    /// the file moves until a shape is added.
    pub fn set_pen(&mut self, pen: Pen) {
        self.pen = pen;
    }

    /// The current bytes — what saving writes.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The shapes, in paint order, groups' members after the group.
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    /// The shape with this `data-id`.
    pub fn shape(&self, id: &str) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id.as_deref() == Some(id))
    }

    /// The `<svg>` element's attributes, in source order.
    pub fn root_attrs(&self) -> &[(String, Option<String>)] {
        &self.node(self.root).attrs
    }

    /// The page: the root's `viewBox` as a box in user units. `None` when
    /// there is none, or it is not four numbers.
    pub fn page(&self) -> Option<Bounds> {
        let value = self.root_attr("viewBox")?;
        let nums: Vec<f64> = value
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<f64>())
            .collect::<Result<_, _>>()
            .ok()?;
        match nums[..] {
            [x, y, width, height] => Some(Bounds {
                x,
                y,
                width,
                height,
            }),
            _ => None,
        }
    }

    /// The box around every shape, in the root's user units: what the page
    /// is fitted to. `None` when no shape has a box — an empty drawing.
    pub fn extent(&self) -> Option<Bounds> {
        self.shapes
            .iter()
            .filter(|s| s.group.is_none())
            .filter_map(|s| self.bounds_of(s))
            .reduce(|a, b| a.union(&b))
    }

    fn root_attr(&self, name: &str) -> Option<&str> {
        self.root_attrs()
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.as_deref())
    }

    /// Hold the drawing to the profile. Empty means it conforms.
    pub fn check(&self) -> Vec<Finding> {
        profile::check(self)
    }

    /// Add a rectangle as the topmost shape, minting its `data-id`. One
    /// splice, one undo step. Returns the id.
    pub fn add_rect(&mut self, rect: Rect) -> Result<String, Error> {
        let f = number::fmt;
        self.add_shape(
            "rect",
            &[
                ("x", f(rect.x)),
                ("y", f(rect.y)),
                ("width", f(rect.width)),
                ("height", f(rect.height)),
            ],
            None,
        )
    }

    /// Add an ellipse filling `bounds` as the topmost shape. Returns the id.
    pub fn add_ellipse(&mut self, bounds: Bounds) -> Result<String, Error> {
        let f = number::fmt;
        self.add_shape(
            "ellipse",
            &[
                ("cx", f(bounds.x + bounds.width / 2.0)),
                ("cy", f(bounds.y + bounds.height / 2.0)),
                ("rx", f(bounds.width / 2.0)),
                ("ry", f(bounds.height / 2.0)),
            ],
            None,
        )
    }

    /// Add a polygon through `points` as the topmost shape, its `points`
    /// written as `x,y` pairs one space apart. Returns the id; an error
    /// for fewer than three points.
    pub fn add_polygon(&mut self, points: &[(f64, f64)]) -> Result<String, Error> {
        if points.len() < 3 {
            return Err(Error::Unsupported {
                gesture: "add polygon with fewer than three points",
                kind: ShapeKind::Polygon,
            });
        }
        let f = number::fmt;
        let list = points
            .iter()
            .map(|&(x, y)| format!("{},{}", f(x), f(y)))
            .collect::<Vec<_>>()
            .join(" ");
        self.add_shape("polygon", &[("points", list)], None)
    }

    /// Add a diamond filling `bounds` — a `<polygon>` through the midpoints
    /// of its sides, top first, clockwise — as the topmost shape. Returns
    /// the id.
    pub fn add_diamond(&mut self, bounds: Bounds) -> Result<String, Error> {
        let (cx, cy) = (
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        );
        self.add_polygon(&[
            (cx, bounds.y),
            (bounds.x + bounds.width, cy),
            (cx, bounds.y + bounds.height),
            (bounds.x, cy),
        ])
    }

    /// Add a line from `(x1, y1)` to `(x2, y2)` as the topmost shape.
    /// Returns the id.
    pub fn add_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) -> Result<String, Error> {
        let f = number::fmt;
        self.add_shape(
            "line",
            &[("x1", f(x1)), ("y1", f(y1)), ("x2", f(x2)), ("y2", f(y2))],
            None,
        )
    }

    /// Add an arrow from `(x1, y1)` to `(x2, y2)` as the topmost shape: a
    /// `<line>` with `data-arrow` saying which ends have a head. The head
    /// itself is drawn by the drawing's `<style>` from that attribute
    /// (`line[data-arrow] { marker-end: url(#arrow) }` and a `<marker>`),
    /// which is the template's, like every stroke and fill; the core
    /// writes what an arrow is, not how it looks. Returns the id.
    pub fn add_arrow(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        heads: Heads,
    ) -> Result<String, Error> {
        let f = number::fmt;
        self.add_shape(
            "line",
            &[
                ("x1", f(x1)),
                ("y1", f(y1)),
                ("x2", f(x2)),
                ("y2", f(y2)),
                ("data-arrow", heads.value().to_string()),
            ],
            None,
        )
    }

    /// Add a freehand stroke as the topmost shape: a `<path>` whose `d` is
    /// the outline `nib` makes of `points` at `widths` (one width for all,
    /// or one per point), with `data-ink` naming the nib and the
    /// centreline and widths beside it (`ink`). Returns the id; an error
    /// for a stroke with no points or no width.
    pub fn add_ink(
        &mut self,
        points: &[(f64, f64)],
        widths: &[f64],
        nib: Nib,
    ) -> Result<String, Error> {
        let d = ink::outline(points, widths, nib).ok_or(Error::Unsupported {
            gesture: "add ink with no points or no width",
            kind: ShapeKind::Path,
        })?;
        self.add_shape(
            "path",
            &[
                ("d", d),
                ("data-ink", nib.value().to_string()),
                ("data-centreline", ink::centreline(points)),
                ("data-widths", ink::widths(widths)),
            ],
            None,
        )
    }

    /// Add a label anchored at `(x, y)` as the topmost shape. `text` is
    /// written escaped; it is plain text, not markup. Returns the id.
    pub fn add_text(&mut self, x: f64, y: f64, text: &str) -> Result<String, Error> {
        let f = number::fmt;
        self.add_shape("text", &[("x", f(x)), ("y", f(y))], Some(text))
    }

    /// Add a note filling `bounds`: a box with a label in it, as one
    /// `<g data-role="note">` of a `<rect>` and a `<text>` wrapped to the
    /// box's inner width, `text` flowed into it. One undo step. Returns
    /// the group's id; [`Drawing::note`] names its members. A note is
    /// resized as one — the box to the new bounds, the label re-wrapped
    /// to it — and deleted as one; its label is re-worded like any other.
    pub fn add_note(&mut self, bounds: Bounds, text: &str) -> Result<String, Error> {
        let f = number::fmt;
        let (group, frame, label) = (self.mint_id(), self.mint_id_after(1), self.mint_id_after(2));
        let inner = (bounds.width - 2.0 * NOTE_PAD).max(1.0);
        let size = self.font_size(None);
        let markup = format!(
            "<g data-role=\"note\" data-id=\"{group}\">\n    <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" data-id=\"{frame}\"{}/>\n    <text x=\"{}\" y=\"{}\" data-width=\"{}\" data-id=\"{label}\">{}</text>\n  </g>",
            f(bounds.x),
            f(bounds.y),
            f(bounds.width),
            f(bounds.height),
            self.pen
                .words("rect")
                .iter()
                .map(|(name, value)| format!(" {name}=\"{}\"", escape(value)))
                .collect::<String>(),
            f(bounds.x + NOTE_PAD),
            f(bounds.y + NOTE_PAD + size),
            f(inner),
            escape(text.trim()),
        );
        self.append_to_root(&markup)?;
        // The words are flowed into the width now the label exists to be
        // measured; folded into the insert.
        let mut steps = 1;
        if self.pen.dash.is_some() {
            self.style_word("[data-dash", style::DASH_RULES, &mut steps)?;
        }
        if !text.trim().is_empty() {
            self.rewrap(&label, Some(inner), &mut steps)?;
        }
        self.follow_page(&mut steps)?;
        Ok(group)
    }

    /// A note's members, when `id` is a note: its box and its label.
    pub fn note(&self, id: &str) -> Option<Note> {
        let group = self.shape(id)?;
        if group.kind != ShapeKind::Group || group.attr("data-role") != Some("note") {
            return None;
        }
        let members: Vec<&Shape> = self.members(id);
        let frame = members
            .iter()
            .find(|m| m.kind == ShapeKind::Rect)?
            .id
            .clone()?;
        let label = members
            .iter()
            .find(|m| m.kind == ShapeKind::Text)?
            .id
            .clone()?;
        Some(Note { frame, label })
    }

    /// A note resized as one: its box to `to`, its label moved to the
    /// box's corner and wrapped to its inner width, one undo step.
    fn resize_note(&mut self, note: Note, to: Bounds) -> Result<(), Error> {
        self.resize(&note.frame, to)?;
        let mut steps = 1;
        let frame = self.shape(&note.frame).expect("just resized");
        // The label sits in the box's own coordinates, being its sibling.
        let (x, y, width) = (
            frame.number("x").unwrap_or(0.0),
            frame.number("y").unwrap_or(0.0),
            frame.number("width").unwrap_or(0.0),
        );
        let size = self.font_size(Some(&note.label));
        let label = self.shape(&note.label).expect("a note's label");
        let updates = [
            ("x", Some(number::fmt(x + NOTE_PAD))),
            ("y", Some(number::fmt(y + NOTE_PAD + size))),
        ];
        self.write_attrs(label.node, &label.attrs.clone(), &updates)?;
        self.fold(&mut steps)?;
        self.rewrap(
            &note.label,
            Some((width - 2.0 * NOTE_PAD).max(1.0)),
            &mut steps,
        )?;
        self.follow_page(&mut steps)
    }

    /// Write one element — `attrs`, then the minted `data-id`, then
    /// `content` between tags or a self-closing tag — as the topmost child
    /// of `<svg>`.
    fn add_shape(
        &mut self,
        tag: &str,
        attrs: &[(&str, String)],
        content: Option<&str>,
    ) -> Result<String, Error> {
        let id = self.mint_id();
        let mut markup = format!("<{tag}");
        for (name, value) in attrs {
            write!(markup, " {name}=\"{}\"", escape(value)).expect("writing to a String");
        }
        write!(markup, " data-id=\"{id}\"").expect("writing to a String");
        let words = self.pen.words(tag);
        for (name, value) in &words {
            write!(markup, " {name}=\"{}\"", escape(value)).expect("writing to a String");
        }
        match content {
            Some(text) => write!(markup, ">{}</{tag}>", escape(text)),
            None => write!(markup, "/>"),
        }
        .expect("writing to a String");
        self.append_to_root(&markup)?;
        let mut steps = 1;
        if words.iter().any(|(name, _)| *name == "data-dash") {
            self.style_word("[data-dash", style::DASH_RULES, &mut steps)?;
        }
        self.follow_page(&mut steps)?;
        Ok(id)
    }

    /// Delete the shape with this `data-id`: one splice, one undo step —
    /// plus, in the same step, the binding taken off any arrow that
    /// pointed at it. The line's indentation is left where it was
    /// (docs/tasks/delete-leaves-its-line.md).
    pub fn delete(&mut self, id: &str) -> Result<(), Error> {
        self.delete_all(&[id])
    }

    /// Delete several shapes as one undo step — a multi-selection's delete.
    /// An id whose shape went with an earlier one's group is skipped.
    pub fn delete_all(&mut self, ids: &[&str]) -> Result<(), Error> {
        let mut steps = 0;
        for id in ids {
            let Some(shape) = self.shape(id) else {
                if steps > 0 {
                    continue;
                }
                return Err(Error::NoSuchShape(id.to_string()));
            };
            let locator = self.locator(shape.node);
            self.editor.delete(&locator).map_err(Error::Edit)?;
            self.reload()?;
            self.fold(&mut steps)?;
        }
        self.unbind_dangling(&mut steps)?;
        self.follow_page(&mut steps)
    }

    /// A shape's extent in the root's user units — its attributes' box
    /// through its transform chain, a `<g>`'s the union of its members'.
    /// `None` for a shape missing the attributes its kind needs, and for an
    /// empty group.
    pub fn bounds(&self, id: &str) -> Option<Bounds> {
        self.shape(id).and_then(|s| self.bounds_of(s))
    }

    fn bounds_of(&self, shape: &Shape) -> Option<Bounds> {
        if shape.kind == ShapeKind::Group {
            return self
                .members_of(shape)
                .filter_map(|m| self.bounds_of(m))
                .reduce(|a, b| a.union(&b));
        }
        let t = self.ctm(shape);
        if t.is_identity() {
            self.local_bounds(shape)
        } else if t.is_axis_aligned() || shape.kind == ShapeKind::Text {
            Some(self.local_bounds(shape)?.transformed(&t))
        } else {
            let subs = geometry::outline(shape)?;
            Bounds::around(
                subs.iter()
                    .flat_map(|s| s.points.iter())
                    .map(|&(x, y)| t.apply(x, y)),
            )
        }
    }

    /// A shape's box in its own coordinates: its attributes', or for a
    /// label the measured one when a host lent its layout.
    fn local_bounds(&self, shape: &Shape) -> Option<Bounds> {
        if shape.kind == ShapeKind::Text
            && let Some(measured) = self.measured_label(shape)
        {
            return Some(measured);
        }
        geometry::bounds(shape)
    }

    /// A label's box by the host's layout, in the label's own coordinates.
    /// `None` without a measurer, or when it lays out nothing. The label
    /// is measured where it is, since a wrapped one's `<tspan>`s carry its
    /// `x` themselves.
    fn measured_label(&self, shape: &Shape) -> Option<Bounds> {
        let content = self.node(shape.node).content_span.clone()?;
        let interior = self.source[content].to_string();
        self.measured_interior(shape, &interior)
    }

    /// The box of `shape` with `interior` as its markup, by the host's
    /// layout; `None` without one, or when it lays out nothing.
    fn measured_interior(&self, shape: &Shape, interior: &str) -> Option<Bounds> {
        let measure = self.measure.as_ref()?;
        let doc = self.label_document(shape, interior);
        *self
            .measured
            .borrow_mut()
            .entry(doc)
            .or_insert_with_key(|doc| measure.measure(doc))
    }

    /// The document a measurer is asked about (see [`Measure::measure`]):
    /// the label where it is, under everything that styles it, with
    /// `interior` as its markup and its own `transform` off.
    fn label_document(&self, shape: &Shape, interior: &str) -> String {
        let mut doc =
            String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"");
        let skip_root = ["xmlns", "width", "height", "viewBox", "preserveAspectRatio"];
        write_attributes(&mut doc, self.root_attrs(), &skip_root);
        doc.push('>');
        let mut next = self.node(self.root).first_child;
        while let Some(id) = next {
            let node = self.node(id);
            next = node.next_sibling;
            if matches!(node.name.as_deref(), Some("style" | "defs")) {
                doc.push_str(&self.source[node.span.clone()]);
            }
        }
        let mut chain = Vec::new();
        let mut group = shape.group.as_deref().and_then(|g| self.shape(g));
        while let Some(g) = group {
            chain.push(g);
            group = g.group.as_deref().and_then(|g| self.shape(g));
        }
        for g in chain.iter().rev() {
            doc.push_str("<g");
            write_attributes(&mut doc, &g.attrs, &["transform"]);
            doc.push('>');
        }
        doc.push_str("<text");
        write_attributes(&mut doc, &shape.attrs, &["transform"]);
        doc.push('>');
        doc.push_str(interior);
        doc.push_str("</text>");
        for _ in &chain {
            doc.push_str("</g>");
        }
        doc.push_str("</svg>");
        doc
    }

    /// The size a label lays out at, in its own user units: its `font-size`
    /// attribute; else what the host's layout says a stylesheet gave it;
    /// else the nominal 12. With `None`, the size a new label directly
    /// under `<svg>` would get.
    pub fn font_size(&self, id: Option<&str>) -> f64 {
        let shape = id.and_then(|id| self.shape(id));
        if let Some(size) = shape
            .and_then(|s| s.attr("font-size"))
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f64>().ok())
        {
            return size;
        }
        self.font(id).map_or(12.0, |f| f.size)
    }

    /// The face and size a label lays out in, as the host's layout
    /// resolved them; `None` without a host layout, or when the label lays
    /// out to nothing. With `None`, a new label's, directly under `<svg>`.
    pub fn font(&self, id: Option<&str>) -> Option<Font> {
        let shape = id.and_then(|id| self.shape(id));
        let doc = match shape {
            Some(shape) => self.label_document(shape, "x"),
            None => {
                let probe = Shape {
                    node: self.root,
                    kind: ShapeKind::Text,
                    id: None,
                    group: None,
                    depth: 0,
                    attrs: Vec::new(),
                    text: None,
                };
                self.label_document(&probe, "x")
            }
        };
        self.measure.as_ref()?.font(&doc)
    }

    /// How wide a line of `shape`'s label would lay out — measured, or by
    /// the nominal six tenths of a font size per character.
    fn line_width(&self, shape: &Shape, line: &str) -> f64 {
        if let Some(b) = self.measured_interior(shape, &escape(line)) {
            return b.width;
        }
        let size = self.font_size(shape.id.as_deref());
        0.6 * size * line.chars().count() as f64
    }

    /// A label's interior for `text`: one escaped run when it is one line
    /// and the label has no `data-width`; otherwise one `<tspan>` per line
    /// — each at the anchor's `x`, each after the first a line down
    /// (`dy="1.2em"`). A `\n` in `text` breaks a line, and the `<tspan>`
    /// it starts carries `data-break="hard"` so it reads back as one; with
    /// `data-width` each such paragraph's words are flowed into that width
    /// besides, the longest word a line of its own when nothing shorter
    /// fits.
    fn flowed(&self, shape: &Shape, text: &str) -> String {
        let width = shape.number("data-width").filter(|w| *w > 0.0);
        let paragraphs: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if width.is_none() && paragraphs.len() <= 1 {
            return escape(text.trim());
        }
        let x = number::fmt(shape.number("x").unwrap_or(0.0));
        // Each line, and whether it starts a paragraph.
        let mut lines: Vec<(String, bool)> = Vec::new();
        for paragraph in paragraphs {
            let first = lines.len();
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if line.is_empty() || width.is_none_or(|w| self.line_width(shape, &candidate) <= w)
                {
                    line = candidate;
                } else {
                    lines.push((std::mem::replace(&mut line, word.to_string()), false));
                }
            }
            if !line.is_empty() {
                lines.push((line, false));
            }
            if let Some(l) = lines.get_mut(first) {
                l.1 = true;
            }
        }
        lines
            .iter()
            .enumerate()
            .map(|(i, (l, starts))| {
                let dy = if i == 0 {
                    String::new()
                } else {
                    format!(" dy=\"{}em\"", number::fmt(geometry::LINE_HEIGHT))
                };
                let hard = if *starts && i > 0 {
                    " data-break=\"hard\""
                } else {
                    ""
                };
                format!("<tspan x=\"{x}\"{dy}{hard}>{}</tspan>", escape(l))
            })
            .collect()
    }

    /// The topmost shape within `tolerance` user units of `(x, y)`, in paint
    /// order — the last one painted wins. A member of a group is returned
    /// itself; [`Drawing::outermost`] is the group a host selects instead.
    pub fn hit(&self, x: f64, y: f64, tolerance: f64) -> Option<&Shape> {
        self.hit_where(x, y, tolerance, |_| true)
    }

    fn hit_where(
        &self,
        x: f64,
        y: f64,
        tolerance: f64,
        wanted: impl Fn(&Shape) -> bool,
    ) -> Option<&Shape> {
        self.shapes.iter().rev().find(|s| {
            if s.kind == ShapeKind::Group || !wanted(s) {
                return false;
            }
            let t = self.ctm(s);
            let Some(inverse) = t.inverse() else {
                return false;
            };
            let (lx, ly) = inverse.apply(x, y);
            let tolerance = tolerance / t.length_scale();
            if s.kind == ShapeKind::Text {
                return self
                    .local_bounds(s)
                    .is_some_and(|b| b.expanded(tolerance).contains(lx, ly));
            }
            crate::hit::hits(s, lx, ly, tolerance)
        })
    }

    /// The outermost group a shape is in, or the shape itself — what a
    /// click on a member selects.
    pub fn outermost(&self, id: &str) -> Option<&Shape> {
        let mut shape = self.shape(id)?;
        while let Some(group) = shape.group.as_deref().and_then(|g| self.shape(g)) {
            shape = group;
        }
        Some(shape)
    }

    /// The shapes directly inside a `<g>`, in paint order. Empty for a
    /// shape that is not a group.
    pub fn members(&self, id: &str) -> Vec<&Shape> {
        self.shape(id)
            .map(|g| self.members_of(g).collect())
            .unwrap_or_default()
    }

    fn members_of<'a>(&'a self, group: &'a Shape) -> impl Iterator<Item = &'a Shape> + 'a {
        self.shapes.iter().filter(move |s| {
            group.kind == ShapeKind::Group && s.group == group.id && group.id.is_some()
        })
    }

    /// A shape's transform in the root's space: its own, then each
    /// enclosing group's.
    fn ctm(&self, shape: &Shape) -> Transform {
        geometry::own_transform(shape).then(&self.parent_ctm(shape))
    }

    fn parent_ctm(&self, shape: &Shape) -> Transform {
        shape
            .group
            .as_deref()
            .and_then(|g| self.shape(g))
            .map(|g| self.ctm(g))
            .unwrap_or_default()
    }

    /// Move a shape by `(dx, dy)` user units: one `set_node_attrs`, one undo
    /// step, every other attribute left in place and in order. A kind with
    /// its position in its attributes has them shifted; a `<path>` or a
    /// `<g>` gets a `translate` composed onto its `transform`. An arrow
    /// bound to the shape follows, in the same step.
    pub fn move_by(&mut self, id: &str, dx: f64, dy: f64) -> Result<(), Error> {
        let mut steps = 0;
        self.shift(id, dx, dy, &mut steps)?;
        self.settle(&[id], &mut steps)
    }

    /// One shape moved, arrows not yet settled.
    fn shift(&mut self, id: &str, dx: f64, dy: f64, steps: &mut usize) -> Result<(), Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        let unsupported = || Error::Unsupported {
            gesture: "move",
            kind: shape.kind,
        };
        // The delta is in the root's units; the attributes are in the
        // shape's own, and a `transform` in its parent's.
        let frame = match shape.kind {
            ShapeKind::Path | ShapeKind::Group => self.parent_ctm(shape),
            _ => self.ctm(shape),
        };
        let (ldx, ldy) = frame
            .inverse()
            .ok_or_else(unsupported)?
            .apply_vector(dx, dy);
        let updates = geometry::moved(shape, ldx, ldy).ok_or_else(unsupported)?;
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)?;
        self.fold(steps)?;
        self.reflow_if_wrapped(id, steps)
    }

    /// A wrapped label's `<tspan>`s carry its `x` themselves: after its
    /// anchor moves, its lines are written again at the new one.
    fn reflow_if_wrapped(&mut self, id: &str, steps: &mut usize) -> Result<(), Error> {
        let Some(shape) = self.shape(id) else {
            return Ok(());
        };
        if shape.kind != ShapeKind::Text || shape.attr("data-width").is_none() {
            return Ok(());
        }
        if let Some(text) = shape.text.clone() {
            let markup = self.flowed(shape, &text);
            self.write_interior(shape.node, &markup)?;
            self.fold(steps)?;
        }
        Ok(())
    }

    /// Move several shapes by `(dx, dy)` as one undo step — a
    /// multi-selection's drag. The ids should not include both a group and
    /// one of its members, which would move the member twice.
    pub fn move_all(&mut self, ids: &[&str], dx: f64, dy: f64) -> Result<(), Error> {
        let mut steps = 0;
        for id in ids {
            self.shift(id, dx, dy, &mut steps)?;
        }
        self.settle(ids, &mut steps)
    }

    /// Fit a shape to `to`, in the root's user units: one `set_node_attrs`,
    /// one undo step. A line keeps its direction, a circle takes the smaller
    /// side, a label goes where the box's corner went and wraps to the
    /// box's width if that changed;
    /// a `<path>` or a `<g>` is scaled by its `transform`, strokes and all.
    /// A shape under a rotation or a skew has no box to fit and is
    /// `Unsupported`. An arrow bound to the shape follows, in the same step.
    pub fn resize(&mut self, id: &str, to: Bounds) -> Result<(), Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        let unsupported = || Error::Unsupported {
            gesture: "resize",
            kind: shape.kind,
        };
        if let Some(note) = self.note(id) {
            return self.resize_note(note, to);
        }
        if shape.kind == ShapeKind::Text {
            // A label goes where its box's corner went, and a box of a
            // different width wraps it to that width.
            let from = self.bounds_of(shape).ok_or_else(unsupported)?;
            self.move_by(id, to.x - from.x, to.y - from.y)?;
            if (to.width - from.width).abs() > 1e-9 {
                let mut steps = 1;
                self.rewrap(id, Some(to.width), &mut steps)?;
                self.follow_page(&mut steps)?;
            }
            return Ok(());
        }
        let updates: Vec<Update> = match shape.kind {
            ShapeKind::Path | ShapeKind::Group => {
                let from = self.bounds_of(shape).ok_or_else(unsupported)?;
                let parent = self.parent_ctm(shape);
                let back = parent.inverse().ok_or_else(unsupported)?;
                // The fit is in root space; conjugate it into the parent's,
                // where the shape's own transform lives.
                let own = geometry::own_transform(shape)
                    .then(&parent)
                    .then(&geometry::fit(from, to))
                    .then(&back);
                // A path takes the scale into its `d` when it can.
                geometry::baked(shape, &own).unwrap_or_else(|| vec![("transform", own.fmt())])
            }
            _ => {
                let t = self.ctm(shape);
                let local = if t.is_identity() {
                    to
                } else if t.is_axis_aligned() {
                    to.transformed(&t.inverse().ok_or_else(unsupported)?)
                } else {
                    return Err(unsupported());
                };
                geometry::resized(shape, local).ok_or_else(unsupported)?
            }
        };
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)?;
        let mut steps = 1;
        self.settle(&[id], &mut steps)
    }

    /// Replace a `<text>`'s characters: one `edit_range` over its interior,
    /// one undo step, the attributes untouched. `text` is plain text,
    /// written escaped — as one run, or flowed into `<tspan>` lines when
    /// the label carries `data-width` (see [`Drawing::set_width`]);
    /// whatever markup the interior held is replaced. A self-closed
    /// `<text/>` is opened. Since the measured box is keyed by the
    /// interior, the label measures afresh. `Unsupported` for what is not
    /// a `<text>`.
    pub fn set_text(&mut self, id: &str, text: &str) -> Result<(), Error> {
        let shape = self.label(id, "set text")?;
        let markup = self.flowed(shape, text);
        self.write_interior(shape.node, &markup)?;
        self.follow_page(&mut 1)
    }

    /// Wrap a label to `width` user units — `data-width` written and its
    /// words re-flowed into `<tspan>` lines — or, with `None`, take the
    /// wrapping off and put the words back on one line. One undo step.
    pub fn set_width(&mut self, id: &str, width: Option<f64>) -> Result<(), Error> {
        self.label(id, "set width")?;
        let mut steps = 0;
        self.rewrap(id, width, &mut steps)?;
        self.follow_page(&mut steps)
    }

    fn rewrap(&mut self, id: &str, width: Option<f64>, steps: &mut usize) -> Result<(), Error> {
        let shape = self.shape(id).expect("a label checked by the caller");
        let updates = [("data-width", width.map(number::fmt))];
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)?;
        self.fold(steps)?;
        let shape = self.shape(id).expect("still there");
        if let Some(text) = shape.text.clone() {
            let markup = self.flowed(shape, &text);
            self.write_interior(shape.node, &markup)?;
            self.fold(steps)?;
        }
        Ok(())
    }

    /// The `<text>` with this id, for a label gesture.
    fn label(&self, id: &str, gesture: &'static str) -> Result<&Shape, Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        if shape.kind != ShapeKind::Text {
            return Err(Error::Unsupported {
                gesture,
                kind: shape.kind,
            });
        }
        Ok(shape)
    }

    /// Replace an element's interior with `markup`, opening a self-closed
    /// one: one `edit_range`.
    fn write_interior(&mut self, node: NodeId, markup: &str) -> Result<(), Error> {
        let node = self.node(node);
        let (start, end, markup) = match node.content_span.clone() {
            Some(content) => (content.start, content.end, markup.to_string()),
            None => {
                // `<text …/>`: the open tag's bytes, less the `/>`.
                let span = node.span.clone();
                let open = self.source[span.clone()]
                    .strip_suffix("/>")
                    .expect("a self-closed element ends in />");
                let tag = node.name.clone().unwrap_or_default();
                (span.start, span.end, format!("{open}>{markup}</{tag}>"))
            }
        };
        self.editor
            .edit_range(start, end, &markup)
            .map_err(Error::Edit)?;
        self.reload()
    }

    /// Wrap shapes in a new `<g>`, minting its `data-id`, which is returned.
    /// The shapes must share a parent (`NotSiblings` otherwise); they are
    /// taken in paint order, and one that is not adjacent to the others is
    /// moved up to them first, since a group paints as one. One undo step.
    /// The `<g>` is written on its own lines around them, the members
    /// indented one level in.
    pub fn group(&mut self, ids: &[&str]) -> Result<String, Error> {
        let mut members = Vec::new();
        for id in ids {
            let shape = self
                .shape(id)
                .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
            if shape.group != self.shape(ids[0]).and_then(|s| s.group.clone()) {
                return Err(Error::NotSiblings);
            }
            let at = self.shapes.iter().position(|s| std::ptr::eq(s, shape));
            members.push((at, id.to_string()));
        }
        members.sort();
        members.dedup();
        let ordered: Vec<String> = members.into_iter().map(|(_, id)| id).collect();
        let group_id = self.mint_id();
        let mut steps = 0;

        // Bring each member up behind the one before it, when something
        // else sits between them.
        for pair in ordered.windows(2) {
            let (prev, cur) = (
                self.shape(&pair[0]).unwrap().node,
                self.shape(&pair[1]).unwrap().node,
            );
            let siblings = self.sibling_shapes(prev);
            let at = siblings.iter().position(|&n| n == prev).unwrap();
            if siblings.get(at + 1) != Some(&cur) {
                let (locator, anchor) = (self.locator(cur), self.locator(prev));
                self.editor
                    .move_after(&locator, &anchor)
                    .map_err(Error::Edit)?;
                self.fold(&mut steps)?;
                self.reload()?;
            }
        }

        let first = self.shape(&ordered[0]).unwrap().node;
        let last = self.shape(ordered.last().unwrap()).unwrap().node;
        let (start, end) = (self.node(first).span.start, self.node(last).span.end);
        let indent = self.indent_before(start);
        let body = &self.source[start..end];
        let markup = if body.contains('\n') {
            let inner = body.replace(&format!("\n{indent}"), &format!("\n{indent}  "));
            format!("<g data-id=\"{group_id}\">\n{indent}  {inner}\n{indent}</g>")
        } else {
            format!("<g data-id=\"{group_id}\">{body}</g>")
        };
        self.editor
            .edit_range(start, end, &markup)
            .map_err(Error::Edit)?;
        self.fold(&mut steps)?;
        self.reload()?;
        Ok(group_id)
    }

    /// Replace a `<g>` with its members, in place; returns their ids. The
    /// group's `transform`, if any, is pushed down onto each member first
    /// — as a shift of a plain shape's attributes, or composed onto a
    /// `transform` — so nothing moves. One undo step. The mirror of
    /// [`Drawing::group`]: members indented one level out, the `<g>`'s
    /// lines gone.
    pub fn ungroup(&mut self, id: &str) -> Result<Vec<String>, Error> {
        let group = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        if group.kind != ShapeKind::Group {
            return Err(Error::Unsupported {
                gesture: "ungroup",
                kind: group.kind,
            });
        }
        let transform = geometry::own_transform(group);
        let ids: Vec<String> = self
            .members(id)
            .iter()
            .filter_map(|m| m.id.clone())
            .collect();
        let mut steps = 0;

        if !transform.is_identity() {
            let count = self.members(id).len();
            for i in 0..count {
                let member = self.members(id)[i].clone();
                let total = geometry::own_transform(&member).then(&transform);
                let updates = geometry::baked(&member, &total)
                    .unwrap_or_else(|| vec![("transform", total.fmt())]);
                self.write_attrs(member.node, &member.attrs, &updates)?;
                self.fold(&mut steps)?;
                if let Some(id) = member.id.as_deref() {
                    self.reflow_if_wrapped(id, &mut steps)?;
                }
            }
        }

        let node = self.shape(id).unwrap().node;
        let span = self.node(node).span.clone();
        let content = self
            .node(node)
            .content_span
            .clone()
            .map(|r| &self.source[r])
            .unwrap_or("");
        let indent = self.indent_before(span.start);
        let (open, close) = (format!("\n{indent}  "), format!("\n{indent}"));
        let body = match content
            .strip_prefix(&open)
            .and_then(|c| c.strip_suffix(&close))
        {
            Some(inner) => inner.replace(&open, &close),
            None => content.trim().to_string(),
        };
        self.editor
            .edit_range(span.start, span.end, &body)
            .map_err(Error::Edit)?;
        self.fold(&mut steps)?;
        self.reload()?;
        // An arrow bound to the group itself has nothing to point at now.
        self.unbind_dangling(&mut steps)?;
        Ok(ids)
    }

    // ----- arrows ----------------------------------------------------------

    /// Where an end of a connector is, in the root's user units. `None`
    /// for what is not one.
    pub fn end_point(&self, id: &str, end: End) -> Option<(f64, f64)> {
        Some(self.connector(id)?.end(end))
    }

    /// A shape as a connector — a `<line>`, or a `<path>` of one straight
    /// or bent segment — in the root's user units: its ends, and its
    /// control point when bent. `None` for what is not one. A host draws
    /// the handles from this: one at each end, one at
    /// [`Connector::midpoint`] for the bend.
    pub fn connector(&self, id: &str) -> Option<Connector> {
        let shape = self.shape(id)?;
        let c = Connector::of(shape)?;
        let t = self.ctm(shape);
        let map = |(x, y): (f64, f64)| t.apply(x, y);
        Some(Connector {
            from: map(c.from),
            to: map(c.to),
            control: c.control.map(map),
        })
    }

    /// Bend a connector so it passes through `(x, y)`, in the root's user
    /// units, half-way along — the gesture of dragging the handle at its
    /// midpoint — or, with `None`, straighten it. A `<line>` bent becomes
    /// a `<path>` with a `Q`, every other attribute kept in place; a
    /// `<path>` straightened becomes a `<line>` again. An end bound to a
    /// shape is re-settled to leave the shape along the new tangent. One
    /// undo step; `Ok(false)` when a `<line>` was asked to be straight,
    /// and nothing was written — a host previewing a drag by undoing the
    /// last application must know there is none to undo. `Unsupported`
    /// for what is not a connector.
    pub fn bend(&mut self, id: &str, through: Option<(f64, f64)>) -> Result<bool, Error> {
        let (shape, c) = self.arrow(id, "bend")?;
        let shape = shape.clone();
        let unsupported = || Error::Unsupported {
            gesture: "bend",
            kind: shape.kind,
        };
        let bent = match through {
            Some((x, y)) => {
                let (lx, ly) = self
                    .ctm(&shape)
                    .inverse()
                    .ok_or_else(unsupported)?
                    .apply(x, y);
                // A bound end leaves its shape toward the control point,
                // so where the ends settle depends on the bend and the
                // bend on the ends; a few rounds meet in the middle, and
                // the handle lands where it was dropped.
                let mut bent = c.through((lx, ly));
                for _ in 0..8 {
                    let next = self.settled(&shape, bent).through((lx, ly));
                    let close = |a: (f64, f64), b: (f64, f64)| {
                        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
                    };
                    if close(next.from, bent.from) && close(next.to, bent.to) {
                        break;
                    }
                    bent = next;
                }
                bent
            }
            None => c.straight(),
        };
        let mut steps = 0;
        match (shape.kind, bent.is_bent()) {
            (ShapeKind::Path, true) => {
                self.write_attrs(shape.node, &shape.attrs, &[("d", Some(bent.d()))])?;
            }
            (ShapeKind::Path, false) => {
                // Straight, it is a `<line>` again: the ends where `d` was.
                let mut attrs = shape.attrs.clone();
                let at = attrs.iter().position(|(k, _)| k == "d").unwrap_or(0);
                attrs.retain(|(k, _)| k != "d");
                for (i, (name, value)) in bent.line_attrs().into_iter().enumerate() {
                    attrs.insert((at + i).min(attrs.len()), (name.to_string(), Some(value)));
                }
                self.retag(shape.node, "line", &attrs)?;
            }
            (_, false) => {
                // A `<line>` is already straight.
                return Ok(false);
            }
            (_, true) => {
                // The `<line>` becomes a `<path>`: `d` where `x1` was, the
                // other attributes as they were.
                let mut attrs = shape.attrs.clone();
                let at = attrs.iter().position(|(k, _)| k == "x1").unwrap_or(0);
                attrs.retain(|(k, _)| !["x1", "y1", "x2", "y2"].contains(&k.as_str()));
                attrs.insert(at.min(attrs.len()), ("d".to_string(), Some(bent.d())));
                self.retag(shape.node, "path", &attrs)?;
            }
        }
        self.fold(&mut steps)?;
        if bent.is_bent() {
            self.style_paths(&mut steps)?;
        }
        self.settle(&[id], &mut steps)?;
        Ok(true)
    }

    /// A bent connector is a `<path>`, and how it looks is the drawing's
    /// `<style>`'s to say. One written before the template styled a
    /// `path` says nothing about it, and SVG's defaults — black fill, no
    /// stroke, no marker — draw a silhouette where the arrow was. Each
    /// top-level `<style>` that selects a `line` and no bare `path` is
    /// widened so its `line` rules select the `path` twin too
    /// ([`style::widened_for_paths`]), folded into the gesture's step.
    fn style_paths(&mut self, steps: &mut usize) -> Result<(), Error> {
        loop {
            let mut next = self.node(self.root).first_child;
            let mut edit = None;
            while let Some(id) = next {
                let node = self.node(id);
                next = node.next_sibling;
                if node.name.as_deref() != Some("style") {
                    continue;
                }
                let Some(content) = node.content_span.clone() else {
                    continue;
                };
                if let Some(widened) = style::widened_for_paths(&self.source[content.clone()]) {
                    edit = Some((content, widened));
                    break;
                }
            }
            let Some((content, widened)) = edit else {
                return Ok(());
            };
            self.editor
                .edit_range(content.start, content.end, &widened)
                .map_err(Error::Edit)?;
            self.reload()?;
            self.fold(steps)?;
        }
    }

    /// A word the drawing's `<style>` has no rule for is drawn as nothing,
    /// so the first shape to take one brings the template's `rules` for it
    /// into the last top-level `<style>`, folded into the gesture's step —
    /// unless some `<style>` already selects `word`, in which case its
    /// author has said what the word means. A drawing with no `<style>` at
    /// all is left as it is: the word is written, and what it looks like
    /// is a stylesheet's to say.
    fn style_word(&mut self, word: &str, rules: &[&str], steps: &mut usize) -> Result<(), Error> {
        let mut sheets = Vec::new();
        let mut next = self.node(self.root).first_child;
        while let Some(id) = next {
            let node = self.node(id);
            next = node.next_sibling;
            if node.name.as_deref() == Some("style")
                && let Some(content) = node.content_span.clone()
            {
                sheets.push(content);
            }
        }
        let mut edit = None;
        for content in sheets {
            match style::with_rules(&self.source[content.clone()], word, rules) {
                Some(added) => edit = Some((content, added)),
                None => return Ok(()),
            }
        }
        let Some((content, added)) = edit else {
            return Ok(());
        };
        self.editor
            .edit_range(content.start, content.end, &added)
            .map_err(Error::Edit)?;
        self.reload()?;
        self.fold(steps)
    }

    /// Rewrite an element as `<tag>` with `attrs`, its interior kept:
    /// one `edit_range` over the element.
    fn retag(
        &mut self,
        node: NodeId,
        tag: &str,
        attrs: &[(String, Option<String>)],
    ) -> Result<(), Error> {
        let node = self.node(node);
        let span = node.span.clone();
        let mut markup = format!("<{tag}");
        write_attributes(&mut markup, attrs, &[]);
        match node.content_span.clone() {
            Some(content) => {
                let inner = &self.source[content];
                write!(markup, ">{inner}</{tag}>").expect("writing to a String");
            }
            None => markup.push_str("/>"),
        }
        self.editor
            .edit_range(span.start, span.end, &markup)
            .map_err(Error::Edit)?;
        self.reload()
    }

    /// Bind an end of a connector to a shape — `data-from` or `data-to`
    /// written, and the end put on the shape's edge, facing the other end
    /// — or, with `None`, unbind it, the end staying put. One undo step.
    /// `Unsupported` for what is not a connector; `NoSuchShape` for a
    /// target that is not there, the arrow itself included.
    pub fn bind(&mut self, id: &str, end: End, target: Option<&str>) -> Result<(), Error> {
        let (shape, _) = self.arrow(id, "bind")?;
        if let Some(target) = target
            && (target == id || self.shape(target).is_none())
        {
            return Err(Error::NoSuchShape(target.to_string()));
        }
        let updates = [(end.binding(), target.map(str::to_string))];
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)?;
        let mut steps = 1;
        self.settle(&[id], &mut steps)
    }

    /// Say how a stroked shape's stroke is broken — `data-dash`, which the
    /// drawing's `<style>` draws — or, with `None`, solid, the word taken
    /// off. On a note it is the note's frame. A drawing whose `<style>`
    /// has no rule for the word gains the template's, in the same step
    /// (docs/proposals/shape-style.md). One undo step; nothing when the
    /// shape already says so. `Unsupported` for what is not stroked — ink,
    /// a label, a group that is not a note, an image.
    pub fn set_dash(&mut self, id: &str, dash: Option<Dash>) -> Result<(), Error> {
        let mut steps = 0;
        self.set_dash_one(id, dash, &mut steps)
    }

    /// `set_dash` over a selection, as one undo step: the stroked shapes
    /// among `ids` take the word, and what is not stroked is left as it
    /// is rather than refused. `NoSuchShape` for an id that is nothing.
    pub fn set_dash_all(&mut self, ids: &[&str], dash: Option<Dash>) -> Result<(), Error> {
        let mut steps = 0;
        for id in ids {
            match self.set_dash_one(id, dash, &mut steps) {
                Err(Error::Unsupported { .. }) => {}
                other => other?,
            }
        }
        Ok(())
    }

    fn set_dash_one(
        &mut self,
        id: &str,
        dash: Option<Dash>,
        steps: &mut usize,
    ) -> Result<(), Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        let shape = match self.note(id) {
            Some(note) => self.shape(&note.frame).expect("a note has its frame"),
            None if shape.is_stroked() => shape,
            None => {
                return Err(Error::Unsupported {
                    gesture: "set_dash",
                    kind: shape.kind,
                });
            }
        };
        if shape.attr("data-dash").map(str::trim) == dash.map(Dash::value) {
            return Ok(());
        }
        let (node, attrs) = (shape.node, shape.attrs.clone());
        let updates = [("data-dash", dash.map(|d| d.value().to_string()))];
        self.write_attrs(node, &attrs, &updates)?;
        self.fold(steps)?;
        if dash.is_some() {
            self.style_word("[data-dash", style::DASH_RULES, steps)?;
        }
        Ok(())
    }

    /// Say which ends of a connector have a head — `data-arrow`, the
    /// profile's word for what an arrow is, which the drawing's `<style>`
    /// draws — or, with `None`, none, which makes it a plain line. A
    /// `marker-start`/`marker-end` the file spelled itself comes off in
    /// the same write, since the word now says it. One `set_node_attrs`,
    /// one undo step; nothing when the shape already says so.
    /// `Unsupported` for what is not a connector.
    pub fn set_heads(&mut self, id: &str, heads: Option<Heads>) -> Result<(), Error> {
        let mut steps = 0;
        self.set_heads_one(id, heads, &mut steps)
    }

    /// `set_heads` over a selection, as one undo step: the connectors
    /// among `ids` take the word, and what is not a connector is left as
    /// it is rather than refused, so a mixed selection takes what
    /// applies. `NoSuchShape` for an id that is nothing.
    pub fn set_heads_all(&mut self, ids: &[&str], heads: Option<Heads>) -> Result<(), Error> {
        let mut steps = 0;
        for id in ids {
            match self.set_heads_one(id, heads, &mut steps) {
                Err(Error::Unsupported { .. }) => {}
                other => other?,
            }
        }
        Ok(())
    }

    fn set_heads_one(
        &mut self,
        id: &str,
        heads: Option<Heads>,
        steps: &mut usize,
    ) -> Result<(), Error> {
        let (shape, _) = self.arrow(id, "set_heads")?;
        let mut updates: Vec<Update> = vec![("data-arrow", heads.map(|h| h.value().to_string()))];
        for spelled in ["marker-start", "marker-end"] {
            if shape.attr(spelled).is_some() {
                updates.push((spelled, None));
            }
        }
        if shape.attr("data-arrow").map(str::trim) == heads.map(Heads::value) && updates.len() == 1
        {
            return Ok(());
        }
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)?;
        self.fold(steps)
    }

    /// Drop an end of a connector at `(x, y)`, in the root's user units:
    /// the end goes there, and is bound to the topmost shape within
    /// `tolerance` of the point — any but the arrow itself — or unbound if
    /// there is none. Returns what it was bound to. One undo step; the
    /// gesture a canvas makes of dragging an endpoint handle.
    pub fn drop_end(
        &mut self,
        id: &str,
        end: End,
        x: f64,
        y: f64,
        tolerance: f64,
    ) -> Result<Option<String>, Error> {
        let (shape, mut c) = self.arrow(id, "drop end")?;
        let unsupported = || Error::Unsupported {
            gesture: "drop end",
            kind: shape.kind,
        };
        let target = self
            .hit_where(x, y, tolerance, |s| s.id.as_deref() != Some(id))
            .and_then(|s| s.id.clone());
        let (lx, ly) = self
            .ctm(shape)
            .inverse()
            .ok_or_else(unsupported)?
            .apply(x, y);
        c.set_end(end, (lx, ly));
        let mut updates = Self::connector_updates(shape, &c);
        updates.push((end.binding(), target.clone()));
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)?;
        let mut steps = 1;
        self.settle(&[id], &mut steps)?;
        Ok(target)
    }

    /// The connector with this id, for a binding gesture.
    fn arrow(&self, id: &str, gesture: &'static str) -> Result<(&Shape, Connector), Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        let c = Connector::of(shape).ok_or(Error::Unsupported {
            gesture,
            kind: shape.kind,
        })?;
        Ok((shape, c))
    }

    /// The attributes that put `shape`'s geometry at `c` — of a `<line>`,
    /// the ends that differ; of a `<path>`, its `d` when it differs.
    /// Empty when nothing would change.
    fn connector_updates(shape: &Shape, c: &Connector) -> Vec<Update> {
        match shape.kind {
            ShapeKind::Line => c
                .line_attrs()
                .into_iter()
                .filter(|(name, value)| shape.number(name).map(number::fmt).as_ref() != Some(value))
                .map(|(name, value)| (name, Some(value)))
                .collect(),
            _ => {
                let d = c.d();
                if shape.attr("d") == Some(d.as_str()) {
                    Vec::new()
                } else {
                    vec![("d", Some(d))]
                }
            }
        }
    }

    /// Put every arrow that `affected` reaches — one of them, or bound to
    /// one of them or to a shape inside one of them — back on its targets'
    /// edges, and then the page around it all, folded into the gesture's
    /// step. Only an end that would move is written.
    fn settle(&mut self, affected: &[&str], steps: &mut usize) -> Result<(), Error> {
        let mut reached: HashSet<String> = HashSet::new();
        for id in affected {
            self.descend(id, &mut reached);
        }
        let arrows: Vec<String> = self
            .shapes
            .iter()
            .filter(|s| Connector::of(s).is_some())
            .filter(|s| {
                let bound = |end: End| s.attr(end.binding());
                (bound(End::From).is_some() || bound(End::To).is_some())
                    && [s.id.as_deref(), bound(End::From), bound(End::To)]
                        .into_iter()
                        .flatten()
                        .any(|id| reached.contains(id))
            })
            .filter_map(|s| s.id.clone())
            .collect();
        for id in arrows {
            let Some(arrow) = self.shape(&id) else {
                continue;
            };
            let updates = self.settled_ends(arrow);
            if !updates.is_empty() {
                self.write_attrs(arrow.node, &arrow.attrs.clone(), &updates)?;
                self.fold(steps)?;
            }
        }
        self.follow_page(steps)
    }

    /// `id` and every shape inside it.
    fn descend(&self, id: &str, into: &mut HashSet<String>) {
        if !into.insert(id.to_string()) {
            return;
        }
        for member in self.members(id) {
            if let Some(member) = member.id.as_deref() {
                self.descend(member, into);
            }
        }
    }

    /// The ends of an arrow moved onto their targets' edges, as the
    /// attributes that put them there; empty when they are there already.
    fn settled_ends(&self, arrow: &Shape) -> Vec<Update> {
        match Connector::of(arrow) {
            Some(c) => Self::connector_updates(arrow, &self.settled(arrow, c)),
            None => Vec::new(),
        }
    }

    /// `local` — `arrow`'s geometry, in its own coordinates — with each
    /// bound end on the segment from its target's centre to what the end
    /// faces: the bend's control point, or the other end's anchor (the
    /// other target's centre, or the other end itself).
    fn settled(&self, arrow: &Shape, local: Connector) -> Connector {
        let t = self.ctm(arrow);
        let Some(back) = t.inverse() else {
            return local;
        };
        let map = |(x, y): (f64, f64)| t.apply(x, y);
        let c = Connector {
            from: map(local.from),
            to: map(local.to),
            control: local.control.map(map),
        };
        let target = |end: End| {
            arrow
                .attr(end.binding())
                .filter(|&id| arrow.id.as_deref() != Some(id))
                .and_then(|id| self.shape(id))
                .and_then(|s| Some((s, self.bounds_of(s)?)))
        };
        let anchor = |end: End| match target(end) {
            Some((_, b)) => (b.x + b.width / 2.0, b.y + b.height / 2.0),
            None => c.end(end),
        };
        let mut settled = local;
        for end in [End::From, End::To] {
            let Some((shape, _)) = target(end) else {
                continue;
            };
            let (px, py) = self.edge(shape, anchor(end), c.facing(anchor(end.other())));
            settled.set_end(end, back.apply(px, py));
        }
        settled
    }

    /// Where the segment from `from` (inside `shape`) to `to` last leaves
    /// the shape's outline, in the root's units; `from` itself when it
    /// never does.
    fn edge(&self, shape: &Shape, from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let mut furthest: Option<f64> = None;
        for sub in self.outline_of(shape) {
            let pts = &sub.points;
            let closing = sub.closed.then(|| (pts[pts.len() - 1], pts[0]));
            for (a, b) in pts.windows(2).map(|w| (w[0], w[1])).chain(closing) {
                let (ex, ey) = (b.0 - a.0, b.1 - a.1);
                let denominator = dx * ey - dy * ex;
                if denominator.abs() < 1e-12 {
                    continue;
                }
                let (wx, wy) = (a.0 - from.0, a.1 - from.1);
                let t = (wx * ey - wy * ex) / denominator;
                let u = (wx * dy - wy * dx) / denominator;
                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    furthest = Some(furthest.map_or(t, |f: f64| f.max(t)));
                }
            }
        }
        match furthest {
            Some(t) => (from.0 + t * dx, from.1 + t * dy),
            None => from,
        }
    }

    /// A shape's silhouette in the root's units: its outline through its
    /// transform chain, a label's box, a group's members'.
    fn outline_of(&self, shape: &Shape) -> Vec<Subpath> {
        if shape.kind == ShapeKind::Group {
            return self
                .members_of(shape)
                .flat_map(|m| self.outline_of(m))
                .collect();
        }
        let t = self.ctm(shape);
        let subs = match shape.kind {
            ShapeKind::Text => self.local_bounds(shape).map(|b| {
                let (x1, y1) = (b.x + b.width, b.y + b.height);
                vec![Subpath {
                    points: vec![(b.x, b.y), (x1, b.y), (x1, y1), (b.x, y1)],
                    closed: true,
                }]
            }),
            _ => geometry::outline(shape),
        };
        subs.unwrap_or_default()
            .into_iter()
            .map(|mut s| {
                for p in &mut s.points {
                    *p = t.apply(p.0, p.1);
                }
                s
            })
            .collect()
    }

    /// Take the binding off every arrow end whose target is gone, folded
    /// into the gesture's step.
    fn unbind_dangling(&mut self, steps: &mut usize) -> Result<(), Error> {
        let dangling: Vec<(String, Vec<Update>)> = self
            .shapes
            .iter()
            .filter(|s| Connector::of(s).is_some())
            .filter_map(|s| {
                let gone: Vec<Update> = [End::From, End::To]
                    .into_iter()
                    .filter(|end| {
                        s.attr(end.binding())
                            .is_some_and(|target| self.shape(target).is_none())
                    })
                    .map(|end| (end.binding(), None))
                    .collect();
                (!gone.is_empty())
                    .then_some(())
                    .and(s.id.clone().map(|id| (id, gone)))
            })
            .collect();
        for (id, updates) in dangling {
            let Some(arrow) = self.shape(&id) else {
                continue;
            };
            self.write_attrs(arrow.node, &arrow.attrs.clone(), &updates)?;
            self.fold(steps)?;
        }
        Ok(())
    }

    /// Change a shape's place in paint order among its sibling shapes: one
    /// `move_before`/`move_after`, one undo step, the whitespace between
    /// siblings kept. `Ok(false)` when it is already there, and nothing was
    /// written.
    pub fn reorder(&mut self, id: &str, order: Order) -> Result<bool, Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        let node = shape.node;
        let siblings = self.sibling_shapes(node);
        let at = siblings
            .iter()
            .position(|&n| n == node)
            .expect("a shape is its own sibling");
        let (anchor, after) = match order {
            Order::Forward if at + 1 < siblings.len() => (siblings[at + 1], true),
            Order::Backward if at > 0 => (siblings[at - 1], false),
            Order::ToFront if at + 1 < siblings.len() => (siblings[siblings.len() - 1], true),
            Order::ToBack if at > 0 => (siblings[0], false),
            _ => return Ok(false),
        };
        let (locator, anchor) = (self.locator(node), self.locator(anchor));
        if after {
            self.editor
                .move_after(&locator, &anchor)
                .map_err(Error::Edit)?;
        } else {
            self.editor
                .move_before(&locator, &anchor)
                .map_err(Error::Edit)?;
        }
        self.reload()?;
        Ok(true)
    }

    /// Undo the last gesture. `Ok(false)` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, Error> {
        let undone = self.editor.undo().map_err(Error::Edit)?.is_some();
        if undone {
            self.reload()?;
        }
        Ok(undone)
    }

    /// Redo the last undone gesture. `Ok(false)` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, Error> {
        let redone = self.editor.redo().map_err(Error::Edit)?.is_some();
        if redone {
            self.reload()?;
        }
        Ok(redone)
    }

    // ----- internals -------------------------------------------------------

    /// Re-read the tree, the shapes and the source after an edit.
    fn reload(&mut self) -> Result<(), Error> {
        self.nodes = self.editor.nodes().map_err(Error::Parse)?;
        self.source = self.editor.source_str().map_err(Error::Parse)?;
        let doc = self
            .nodes
            .iter()
            .find(|n| n.kind == Kind::Doc)
            .ok_or(Error::NotSvg)?;
        // The document element: the first element child of the document
        // node. A prolog, a comment or a doctype before it is fine.
        let mut next = doc.first_child;
        let mut root = None;
        while let Some(id) = next {
            let node = self.node(id);
            next = node.next_sibling;
            if node.kind == Kind::Container {
                root = Some(node);
                break;
            }
        }
        let root = root.ok_or(Error::NotSvg)?;
        if root.name.as_deref() != Some("svg") {
            return Err(Error::NotSvg);
        }
        self.root = root.id;
        self.shapes = shape::read(&self.nodes, self.root);
        Ok(())
    }

    fn node(&self, id: NodeId) -> &FlatNode {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .expect("a node id from this tree")
    }

    /// twig's index-path locator for a node: each step is the node's index
    /// among *all* its parent's children, whitespace text included.
    fn locator(&self, id: NodeId) -> String {
        let mut steps = Vec::new();
        let mut current = id;
        while let Some(parent) = self.node(current).parent {
            let mut index = 0;
            let mut next = self.node(parent).first_child;
            while let Some(sibling) = next {
                if sibling == current {
                    break;
                }
                index += 1;
                next = self.node(sibling).next_sibling;
            }
            steps.push(index.to_string());
            current = parent;
        }
        steps.reverse();
        steps.join(".")
    }

    /// The `<svg>` element's last element child, if any.
    fn last_element_child(&self) -> Option<NodeId> {
        let mut last = None;
        let mut next = self.node(self.root).first_child;
        while let Some(id) = next {
            let node = self.node(id);
            if node.kind == Kind::Container {
                last = Some(id);
            }
            next = node.next_sibling;
        }
        last
    }

    /// Insert markup as the topmost child of `<svg>`, on its own line, in
    /// one splice. After the last element when there is one; otherwise as the
    /// first child, which is where twig's `insert_child` puts it.
    fn append_to_root(&mut self, markup: &str) -> Result<(), Error> {
        match self.last_element_child() {
            Some(last) => {
                let locator = self.locator(last);
                self.editor
                    .insert_after(&locator, &format!("\n  {markup}"))
                    .map_err(Error::Edit)?;
            }
            None => {
                let locator = self.locator(self.root);
                let empty = self.node(self.root).first_child.is_none();
                let text = if empty {
                    format!("\n  {markup}\n")
                } else {
                    format!("\n  {markup}")
                };
                self.editor
                    .insert_child(&locator, 0, &text)
                    .map_err(Error::Edit)?;
            }
        }
        self.reload()
    }

    /// Fit the page around the shapes, [`PAGE_MARGIN`] out from their box,
    /// folded into the gesture's step: the `viewBox` rewritten (or written,
    /// when the root had none), and `width` and `height` with it when each
    /// is a plain number, in `px` or unitless, so a viewer still shows one
    /// user unit per pixel. Nothing is written when the page already fits,
    /// when there is no shape to fit to, or when the gesture wrote nothing
    /// — a no-op gesture is not the moment to fit a page a hand left loose.
    fn follow_page(&mut self, steps: &mut usize) -> Result<(), Error> {
        if *steps == 0 {
            return Ok(());
        }
        let Some(extent) = self.extent() else {
            return Ok(());
        };
        let f = number::fmt;
        let page = Bounds {
            x: extent.x - PAGE_MARGIN,
            y: extent.y - PAGE_MARGIN,
            width: extent.width + 2.0 * PAGE_MARGIN,
            height: extent.height + 2.0 * PAGE_MARGIN,
        };
        let view_box = format!(
            "{} {} {} {}",
            f(page.x),
            f(page.y),
            f(page.width),
            f(page.height)
        );
        let attrs = self.node(self.root).attrs.clone();
        let mut updates: Vec<Update> = Vec::new();
        if self.root_attr("viewBox") != Some(view_box.as_str()) {
            updates.push(("viewBox", Some(view_box)));
        }
        for (name, length) in [("width", page.width), ("height", page.height)] {
            let Some(current) = self.root_attr(name) else {
                continue;
            };
            let (digits, unit) = match current.strip_suffix("px") {
                Some(digits) => (digits, "px"),
                None => (current, ""),
            };
            if digits.trim().parse::<f64>().is_err() {
                continue;
            }
            let fitted = format!("{}{unit}", f(length));
            if current != fitted {
                updates.push((name, Some(fitted)));
            }
        }
        if updates.is_empty() {
            return Ok(());
        }
        self.write_attrs(self.root, &attrs, &updates)?;
        self.fold(steps)
    }

    /// Count one twig operation of a gesture; from the second on, fold it
    /// into the undo step before it, so the gesture undoes as one.
    fn fold(&mut self, steps: &mut usize) -> Result<(), Error> {
        *steps += 1;
        if *steps > 1 {
            self.editor.coalesce_last_undo().map_err(Error::Edit)?;
        }
        Ok(())
    }

    /// The whitespace between the start of the line and `offset`, when
    /// that is all the line holds so far — a node's indentation.
    fn indent_before(&self, offset: usize) -> &str {
        let line_start = self.source[..offset].rfind('\n').map_or(0, |i| i + 1);
        let prefix = &self.source[line_start..offset];
        if prefix.chars().all(|c| c == ' ' || c == '\t') {
            prefix
        } else {
            ""
        }
    }

    /// Write `updates` over `attrs` — replacing a value in place, appending
    /// a name not yet present, taking off one updated to `None` — through
    /// one `set_node_attrs`.
    fn write_attrs(
        &mut self,
        node: NodeId,
        attrs: &[(String, Option<String>)],
        updates: &[Update],
    ) -> Result<(), Error> {
        let mut merged: Vec<(String, Option<String>)> = attrs.to_vec();
        for (name, value) in updates {
            match (merged.iter().position(|(k, _)| k == name), value) {
                (Some(at), Some(value)) => merged[at].1 = Some(value.clone()),
                (Some(at), None) => {
                    merged.remove(at);
                }
                (None, Some(value)) => merged.push((name.to_string(), Some(value.clone()))),
                (None, None) => {}
            }
        }
        let borrowed: Vec<(&str, Option<&str>)> = merged
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_deref()))
            .collect();
        self.editor
            .set_node_attrs(node, &borrowed)
            .map_err(Error::Edit)?;
        self.reload()
    }

    /// The shape elements among a node's siblings (itself included), in
    /// source order — what a reorder moves among.
    fn sibling_shapes(&self, node: NodeId) -> Vec<NodeId> {
        let parent = self.node(node).parent.expect("a shape has a parent");
        let mut out = Vec::new();
        let mut next = self.node(parent).first_child;
        while let Some(id) = next {
            let n = self.node(id);
            next = n.next_sibling;
            if n.kind == Kind::Container
                && n.name.as_deref().and_then(ShapeKind::from_tag).is_some()
            {
                out.push(id);
            }
        }
        out
    }

    /// The next free `s<n>` id: one past the largest such id in the file.
    fn mint_id(&self) -> String {
        self.mint_id_after(0)
    }

    /// The id `n` after the next free one, for a gesture that mints
    /// several in one splice.
    fn mint_id_after(&self, n: u64) -> String {
        let taken = self
            .shapes
            .iter()
            .filter_map(|s| s.id.as_deref())
            .filter_map(|id| id.strip_prefix('s'))
            .filter_map(|n| n.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("s{}", taken + 1 + n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n</svg>\n";

    #[test]
    fn refuses_what_is_not_an_svg() {
        assert!(matches!(Drawing::open("<html></html>"), Err(Error::NotSvg)));
        assert!(matches!(Drawing::open("<svg"), Err(Error::Parse(_))));
    }

    #[test]
    fn a_prolog_and_a_comment_before_the_root_are_fine() {
        let d = Drawing::open("<?xml version=\"1.0\"?>\n<!-- x -->\n<svg viewBox=\"0 0 1 1\"/>")
            .unwrap();
        assert_eq!(d.shapes().len(), 0);
        assert_eq!(d.root_attrs()[0].0, "viewBox");
    }

    #[test]
    fn add_writes_one_line_and_mints_the_next_id() {
        let mut d = Drawing::open(EMPTY).unwrap();
        let a = d
            .add_rect(Rect {
                x: 10.0,
                y: 10.5,
                width: 80.0,
                height: 40.125,
            })
            .unwrap();
        let b = d
            .add_rect(Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            })
            .unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("s1", "s2"));
        assert_eq!(
            d.source(),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-16 -16 122 82.625\" data-diaryx-drawing=\"1\">\n  <rect x=\"10\" y=\"10.5\" width=\"80\" height=\"40.125\" data-id=\"s1\"/>\n  <rect x=\"0\" y=\"0\" width=\"1\" height=\"1\" data-id=\"s2\"/>\n</svg>\n"
        );
        assert_eq!(d.shapes().len(), 2);
        assert_eq!(d.shape("s2").unwrap().number("width"), Some(1.0));
        assert!(d.check().is_empty());
    }

    #[test]
    fn each_kind_adds_as_one_line() {
        let mut d = Drawing::open(EMPTY).unwrap();
        d.add_ellipse(Bounds {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 4.0,
        })
        .unwrap();
        d.add_line(1.0, 2.0, 3.0, 4.0).unwrap();
        d.add_text(5.0, 6.0, "a < b & \"c\"").unwrap();
        assert_eq!(
            d.source(),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-16 -19.6 116.2 44\" data-diaryx-drawing=\"1\">\n  <ellipse cx=\"5\" cy=\"2\" rx=\"5\" ry=\"2\" data-id=\"s1\"/>\n  <line x1=\"1\" y1=\"2\" x2=\"3\" y2=\"4\" data-id=\"s2\"/>\n  <text x=\"5\" y=\"6\" data-id=\"s3\">a &lt; b &amp; &quot;c&quot;</text>\n</svg>\n"
        );
        assert!(d.check().is_empty());
        assert_eq!(d.shapes().len(), 3);
    }

    #[test]
    fn add_into_an_empty_root_opens_it() {
        let mut d = Drawing::open("<svg viewBox=\"0 0 1 1\"></svg>").unwrap();
        d.add_rect(Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        })
        .unwrap();
        assert_eq!(
            d.source(),
            "<svg viewBox=\"-15 -14 35 36\">\n  <rect x=\"1\" y=\"2\" width=\"3\" height=\"4\" data-id=\"s1\"/>\n</svg>"
        );
    }

    #[test]
    fn a_self_closed_root_has_nowhere_to_insert() {
        let mut d = Drawing::open("<svg viewBox=\"0 0 1 1\"/>").unwrap();
        assert!(matches!(
            d.add_rect(Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0
            }),
            Err(Error::Edit(twig::Error::NotEditable))
        ));
    }

    #[test]
    fn delete_and_undo_are_one_step_each_and_touch_nothing_else() {
        let src = "<svg viewBox=\"0 0 9 9\" data-diaryx-drawing=\"1\">\n  <!-- keep me -->\n  <rect x=\"1\" y=\"1\" width=\"2\" height=\"2\" data-id=\"s1\"/>\n  <g data-id=\"s2\"><circle cx=\"5\" cy=\"5\" r=\"1\" data-id=\"s3\"/></g>\n</svg>\n";
        let mut d = Drawing::open(src).unwrap();
        assert_eq!(
            d.shapes()
                .iter()
                .map(|s| s.id.clone().unwrap())
                .collect::<Vec<_>>(),
            ["s1", "s2", "s3"]
        );
        assert_eq!(d.shape("s3").unwrap().group.as_deref(), Some("s2"));
        assert_eq!(d.shape("s3").unwrap().depth, 1);

        d.delete("s1").unwrap();
        assert_eq!(
            d.source(),
            "<svg viewBox=\"-12 -12 34 34\" data-diaryx-drawing=\"1\">\n  <!-- keep me -->\n  \n  <g data-id=\"s2\"><circle cx=\"5\" cy=\"5\" r=\"1\" data-id=\"s3\"/></g>\n</svg>\n"
        );
        assert!(d.shape("s1").is_none());
        assert!(matches!(d.delete("s1"), Err(Error::NoSuchShape(_))));

        assert!(d.undo().unwrap());
        assert_eq!(d.source(), src);
        assert!(d.shape("s1").is_some());
        assert!(!d.undo().unwrap());
        assert!(d.redo().unwrap());
        assert!(d.shape("s1").is_none());
    }

    const SCENE: &str = "<svg viewBox=\"0 0 100 100\" data-diaryx-drawing=\"1\">\n  <defs><marker id=\"m\"/></defs>\n  <rect x=\"10\" y=\"10\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>\n  <circle cx=\"50\" cy=\"50\" r=\"5\" data-id=\"s2\"/>\n  <line x1=\"30\" y1=\"15\" x2=\"45\" y2=\"50\" data-id=\"s3\"/>\n</svg>\n";

    fn ids(d: &Drawing) -> Vec<String> {
        d.shapes().iter().map(|s| s.id.clone().unwrap()).collect()
    }

    #[test]
    fn the_page_follows_the_shapes() {
        let mut d = Drawing::fresh();
        assert_eq!(
            d.page(),
            Some(Bounds {
                x: 0.0,
                y: 0.0,
                width: 640.0,
                height: 400.0
            })
        );
        assert_eq!(d.extent(), None, "nothing to fit to");
        let a = d
            .add_rect(Rect {
                x: 100.0,
                y: 50.0,
                width: 80.0,
                height: 40.0,
            })
            .unwrap();
        // The margin out from the rect, and `width`/`height` with it.
        assert!(
            d.source()
                .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"84 34 112 72\" width=\"112\" height=\"72\" color=\"#222\" data-diaryx-drawing=\"1\">"),
            "{}",
            d.source()
        );
        assert!(d.check().is_empty());
        assert!(d.undo().unwrap(), "the add and the page are one step");
        assert_eq!(d.source(), profile::TEMPLATE);
        assert!(d.redo().unwrap());

        // A move past the edge grows the page; a move within an extent
        // some other shape sets leaves it alone.
        d.move_by(&a, -200.0, 0.0).unwrap();
        assert_eq!(
            d.page(),
            Some(Bounds {
                x: -116.0,
                y: 34.0,
                width: 112.0,
                height: 72.0
            })
        );
        let b = d
            .add_rect(Rect {
                x: 200.0,
                y: 200.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        let c = d
            .add_rect(Rect {
                x: 0.0,
                y: 100.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        let fitted = d.source().to_string();
        d.move_by(&c, 5.0, 5.0).unwrap();
        assert_eq!(
            d.source(),
            fitted.replace("<rect x=\"0\" y=\"100\"", "<rect x=\"5\" y=\"105\""),
            "the page already fits, so only the rect's line changes"
        );
        // And it shrinks: the far corner pulled in pulls the page in.
        d.move_by(&b, -10.0, -10.0).unwrap();
        assert_eq!(
            d.page(),
            Some(Bounds {
                x: -116.0,
                y: 34.0,
                width: 332.0,
                height: 182.0
            })
        );

        // Deleting the last shape leaves the page where it was: an empty
        // drawing has nothing to fit to, and keeps its size.
        d.delete_all(&[&a, &b, &c]).unwrap();
        assert_eq!(d.extent(), None);
        assert_eq!(
            d.page(),
            Some(Bounds {
                x: -116.0,
                y: 34.0,
                width: 332.0,
                height: 182.0
            })
        );
        assert!(d.undo().unwrap(), "one step");
        assert_eq!(d.shapes().len(), 3);
    }

    #[test]
    fn the_page_is_written_where_there_was_none_and_lengths_keep_their_unit() {
        // No viewBox: the first gesture gives the drawing one, appended.
        let mut d = Drawing::open("<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>").unwrap();
        assert_eq!(d.page(), None);
        d.add_line(0.0, 0.0, 10.0, 10.0).unwrap();
        assert_eq!(
            d.source(),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-16 -16 42 42\">\n  <line x1=\"0\" y1=\"0\" x2=\"10\" y2=\"10\" data-id=\"s1\"/>\n</svg>"
        );
        // A `px` length follows in `px`; a percentage is not a page size
        // and is left alone. Commas in a viewBox parse.
        let mut d = Drawing::open(
            "<svg viewBox=\"0,0,100,100\" width=\"100px\" height=\"100%\"><rect x=\"0\" y=\"0\" width=\"10\" height=\"10\" data-id=\"r\"/></svg>",
        )
        .unwrap();
        assert_eq!(
            d.page(),
            Some(Bounds {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0
            })
        );
        d.move_by("r", 1.0, 0.0).unwrap();
        assert!(
            d.source()
                .starts_with("<svg viewBox=\"-15 -16 42 42\" width=\"42px\" height=\"100%\">"),
            "{}",
            d.source()
        );
    }

    #[test]
    fn the_extent_is_through_transforms() {
        let d = Drawing::open(
            "<svg viewBox=\"0 0 1 1\"><g data-id=\"g\" transform=\"translate(100 0) scale(2)\"><rect x=\"0\" y=\"0\" width=\"10\" height=\"10\" data-id=\"r\"/></g><circle cx=\"0\" cy=\"0\" r=\"5\" data-id=\"c\"/></svg>",
        )
        .unwrap();
        assert_eq!(
            d.extent(),
            Some(Bounds {
                x: -5.0,
                y: -5.0,
                width: 125.0,
                height: 25.0
            })
        );
    }

    #[test]
    fn move_rewrites_the_position_and_nothing_else() {
        let mut d = Drawing::open(SCENE).unwrap();
        d.move_by("s1", 5.0, -2.5).unwrap();
        // The rect's line, and the page around the shapes: nothing else.
        assert_eq!(
            d.source(),
            SCENE
                .replace(
                    "<rect x=\"10\" y=\"10\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>",
                    "<rect x=\"15\" y=\"7.5\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>"
                )
                .replace("viewBox=\"0 0 100 100\"", "viewBox=\"-1 -8.5 72 79.5\"")
        );
        assert_eq!(
            d.bounds("s1"),
            Some(Bounds {
                x: 15.0,
                y: 7.5,
                width: 20.0,
                height: 10.0
            })
        );
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), SCENE);
        assert!(d.check().is_empty());
    }

    #[test]
    fn hit_is_the_topmost_shape_under_the_pointer() {
        let mut d = Drawing::open(SCENE).unwrap();
        assert_eq!(
            d.hit(15.0, 15.0, 0.0).and_then(|s| s.id.as_deref()),
            Some("s1")
        );
        assert_eq!(
            d.hit(52.0, 52.0, 0.0).and_then(|s| s.id.as_deref()),
            Some("s2")
        );
        assert_eq!(d.hit(90.0, 90.0, 0.0), None);
        // The line ends on the circle's edge; the line was painted last and
        // wins there, until the circle is brought to the front.
        assert_eq!(
            d.hit(45.0, 50.0, 1.0).and_then(|s| s.id.as_deref()),
            Some("s3")
        );
        d.reorder("s2", Order::ToFront).unwrap();
        assert_eq!(
            d.hit(45.0, 50.0, 1.0).and_then(|s| s.id.as_deref()),
            Some("s2")
        );
    }

    #[test]
    fn resize_fits_the_box() {
        let mut d = Drawing::open(SCENE).unwrap();
        d.resize(
            "s2",
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            },
        )
        .unwrap();
        assert_eq!(d.shape("s2").unwrap().attr("r"), Some("5"));
        assert_eq!(d.shape("s2").unwrap().attr("cx"), Some("5"));
        d.resize(
            "s3",
            Bounds {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
        )
        .unwrap();
        assert_eq!(d.shape("s3").unwrap().number("x2"), Some(4.0));
        assert!(d.undo().unwrap());
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), SCENE);
    }

    #[test]
    fn a_path_moves_by_rewriting_its_d_and_is_hit_there() {
        let src = "<svg viewBox=\"0 0 100 100\">\n  <path d=\"M10 10 h20 v10 z\" data-id=\"p\"/>\n</svg>\n";
        let mut d = Drawing::open(src).unwrap();
        assert_eq!(
            d.bounds("p"),
            Some(Bounds {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 10.0
            })
        );
        d.move_by("p", 5.0, 5.0).unwrap();
        let moved = "<svg viewBox=\"-1 -1 52 42\">\n  <path d=\"M15 15 L35 15 L35 25 Z\" data-id=\"p\"/>\n</svg>\n";
        assert_eq!(
            d.source(),
            moved,
            "absolute commands, in the profile's format"
        );
        assert_eq!(d.bounds("p").map(|b| (b.x, b.y)), Some((15.0, 15.0)));
        assert_eq!(
            d.hit(30.0, 19.0, 0.0).and_then(|s| s.id.as_deref()),
            Some("p")
        );
        assert_eq!(d.hit(12.0, 12.0, 0.0), None, "where it was");
        d.move_by("p", -5.0, -5.0).unwrap();
        assert_eq!(
            d.source(),
            "<svg viewBox=\"-6 -6 52 42\">\n  <path d=\"M10 10 L30 10 L30 20 Z\" data-id=\"p\"/>\n</svg>\n",
            "moved back: the hand's spelling is gone, the shape is not"
        );
        // Under a rotation there is nothing to bake into: the move goes
        // onto the transform, and the hit is through it.
        let mut r = Drawing::open(
            "<svg viewBox=\"0 0 100 100\"><path d=\"M10 10 h20 v10 z\" data-id=\"p\" transform=\"rotate(90)\"/></svg>",
        )
        .unwrap();
        r.move_by("p", 5.0, 5.0).unwrap();
        assert_eq!(
            r.shape("p").unwrap().attrs[2..],
            [(
                "transform".to_string(),
                Some("matrix(0 1 -1 0 5 5)".to_string())
            )]
        );
        assert_eq!(
            r.hit(-10.0, 30.0, 0.0).and_then(|s| s.id.as_deref()),
            Some("p")
        );
        assert!(matches!(
            d.move_by("zz", 1.0, 1.0),
            Err(Error::NoSuchShape(_))
        ));
    }

    #[test]
    fn a_path_resizes_by_a_scale_about_its_corner() {
        let mut d = Drawing::open(
            "<svg viewBox=\"0 0 100 100\"><path d=\"M10 10 h20 v10 z\" data-id=\"p\"/></svg>",
        )
        .unwrap();
        d.resize(
            "p",
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 40.0,
                height: 40.0,
            },
        )
        .unwrap();
        assert_eq!(
            d.shape("p").unwrap().attrs,
            [
                ("d".to_string(), Some("M0 0 L40 0 L40 40 Z".to_string())),
                ("data-id".to_string(), Some("p".to_string()))
            ],
            "the scale is baked into the d"
        );
        assert_eq!(
            d.bounds("p"),
            Some(Bounds {
                x: 0.0,
                y: 0.0,
                width: 40.0,
                height: 40.0
            })
        );
    }

    #[test]
    fn a_resized_group_ungroups_into_plain_attributes() {
        let mut d = Drawing::open(
            "<svg viewBox=\"0 0 100 100\">\n  <g data-id=\"g\">\n    <rect x=\"0\" y=\"0\" width=\"10\" height=\"10\" data-id=\"r\"/>\n    <circle cx=\"20\" cy=\"5\" r=\"5\" data-id=\"c\"/>\n    <text x=\"0\" y=\"20\" font-size=\"10\" data-id=\"t\">Hi</text>\n  </g>\n</svg>\n",
        )
        .unwrap();
        // Double the group; its transform is a matrix over three shapes.
        let from = d.bounds("g").unwrap();
        d.resize(
            "g",
            Bounds {
                x: from.x,
                y: from.y,
                width: from.width * 2.0,
                height: from.height * 2.0,
            },
        )
        .unwrap();
        assert_eq!(
            d.shape("g").unwrap().attr("transform"),
            Some("matrix(2 0 0 2 0 0)")
        );
        let before: Vec<_> = ["r", "c", "t"].iter().map(|id| d.bounds(id)).collect();
        // Ungrouped, each shape carries the scale in its own attributes —
        // the circle its radius, the label its font-size — and no transform.
        d.ungroup("g").unwrap();
        assert_eq!(
            d.source(),
            "<svg viewBox=\"-16 -16 82 76\">\n  <rect x=\"0\" y=\"0\" width=\"20\" height=\"20\" data-id=\"r\"/>\n  <circle cx=\"40\" cy=\"10\" r=\"10\" data-id=\"c\"/>\n  <text x=\"0\" y=\"40\" font-size=\"20\" data-id=\"t\">Hi</text>\n</svg>\n"
        );
        let after: Vec<_> = ["r", "c", "t"].iter().map(|id| d.bounds(id)).collect();
        assert_eq!(after, before, "nothing moved");
        // Squashed instead, the circle and the label have no attributes
        // for an unequal scale and keep a transform.
        assert!(d.undo().unwrap());
        assert!(d.undo().unwrap());
        d.resize(
            "g",
            Bounds {
                x: from.x,
                y: from.y,
                width: from.width * 2.0,
                height: from.height,
            },
        )
        .unwrap();
        d.ungroup("g").unwrap();
        assert_eq!(d.shape("r").unwrap().attr("width"), Some("20"));
        assert_eq!(
            d.shape("c").unwrap().attr("transform"),
            Some("matrix(2 0 0 1 0 0)")
        );
        assert_eq!(
            d.shape("t").unwrap().attr("transform"),
            Some("matrix(2 0 0 1 0 0)")
        );
    }

    const GROUPED: &str = "<svg viewBox=\"0 0 100 100\" data-diaryx-drawing=\"1\">\n  <g data-id=\"s1\" transform=\"translate(10 0)\">\n    <rect x=\"0\" y=\"0\" width=\"10\" height=\"10\" data-id=\"s2\"/>\n    <path d=\"M20 0 h10\" data-id=\"s3\"/>\n  </g>\n</svg>\n";

    #[test]
    fn a_group_is_its_members_through_its_transform() {
        let mut d = Drawing::open(GROUPED).unwrap();
        assert_eq!(
            d.bounds("s1"),
            Some(Bounds {
                x: 10.0,
                y: 0.0,
                width: 30.0,
                height: 10.0
            })
        );
        assert_eq!(
            d.bounds("s2").map(|b| b.x),
            Some(10.0),
            "a member's box is in root units"
        );
        assert_eq!(
            d.hit(15.0, 5.0, 0.0).and_then(|s| s.id.as_deref()),
            Some("s2")
        );
        assert_eq!(d.outermost("s2").and_then(|s| s.id.as_deref()), Some("s1"));
        assert_eq!(d.outermost("s1").and_then(|s| s.id.as_deref()), Some("s1"));
        assert_eq!(d.members("s1").len(), 2);
        assert!(d.members("s2").is_empty());

        // Moving a member: the delta is in root units, its attributes in the
        // group's, and a translate keeps them equal.
        d.move_by("s2", 1.0, 2.0).unwrap();
        assert_eq!(d.shape("s2").unwrap().attr("x"), Some("1"));
        assert!(d.undo().unwrap());
        // Moving the group composes onto its transform.
        d.move_by("s1", 5.0, 5.0).unwrap();
        assert_eq!(
            d.shape("s1").unwrap().attr("transform"),
            Some("translate(15 5)")
        );
        assert_eq!(d.bounds("s2").map(|b| (b.x, b.y)), Some((15.0, 5.0)));
        assert!(d.undo().unwrap());
        // Resizing the group scales it.
        d.resize(
            "s1",
            Bounds {
                x: 10.0,
                y: 0.0,
                width: 60.0,
                height: 10.0,
            },
        )
        .unwrap();
        assert_eq!(
            d.shape("s1").unwrap().attr("transform"),
            Some("matrix(2 0 0 1 10 0)")
        );
        assert_eq!(d.bounds("s1").map(|b| b.width), Some(60.0));
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), GROUPED);
    }

    #[test]
    fn a_member_under_a_scale_moves_by_the_root_delta() {
        let mut d = Drawing::open("<svg viewBox=\"0 0 100 100\"><g data-id=\"g\" transform=\"scale(2)\"><rect x=\"0\" y=\"0\" width=\"5\" height=\"5\" data-id=\"r\"/></g></svg>").unwrap();
        d.move_by("r", 10.0, 10.0).unwrap();
        assert_eq!(
            d.shape("r").unwrap().attr("x"),
            Some("5"),
            "half, in the group's units"
        );
        assert_eq!(d.bounds("r").map(|b| b.x), Some(10.0));
        d.resize(
            "r",
            Bounds {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            },
        )
        .unwrap();
        assert_eq!(d.shape("r").unwrap().attr("width"), Some("10"));
        let rotated = Drawing::open(
            "<svg viewBox=\"0 0 100 100\"><rect x=\"0\" y=\"0\" width=\"5\" height=\"5\" transform=\"rotate(45)\" data-id=\"r\"/></svg>",
        );
        let mut rotated = rotated.unwrap();
        assert!(matches!(
            rotated.resize(
                "r",
                Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0
                }
            ),
            Err(Error::Unsupported {
                gesture: "resize",
                ..
            })
        ));
        assert_eq!(
            rotated.hit(0.0, 3.0, 0.0).and_then(|s| s.id.as_deref()),
            Some("r"),
            "on the rotated square"
        );
        assert_eq!(rotated.hit(3.0, 0.0, 0.0), None, "off it, in its old box");
    }

    #[test]
    fn group_wraps_the_members_and_ungroup_is_its_mirror() {
        let mut d = Drawing::open(SCENE).unwrap();
        let g = d.group(&["s3", "s1"]).unwrap();
        assert_eq!(g, "s4");
        assert_eq!(
            d.source(),
            "<svg viewBox=\"0 0 100 100\" data-diaryx-drawing=\"1\">\n  <defs><marker id=\"m\"/></defs>\n  <g data-id=\"s4\">\n    <rect x=\"10\" y=\"10\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>\n    <line x1=\"30\" y1=\"15\" x2=\"45\" y2=\"50\" data-id=\"s3\"/>\n  </g>\n  <circle cx=\"50\" cy=\"50\" r=\"5\" data-id=\"s2\"/>\n</svg>\n",
            "the line came up behind the rect; the group is where the rect was"
        );
        assert_eq!(ids(&d), ["s4", "s1", "s3", "s2"]);
        assert_eq!(d.shape("s3").unwrap().group.as_deref(), Some("s4"));
        assert!(d.check().is_empty());
        assert!(d.undo().unwrap(), "one step, though it was two edits");
        assert_eq!(d.source(), SCENE);
        assert!(d.redo().unwrap());

        assert_eq!(d.ungroup("s4").unwrap(), ["s1", "s3"]);
        assert_eq!(
            d.source(),
            "<svg viewBox=\"0 0 100 100\" data-diaryx-drawing=\"1\">\n  <defs><marker id=\"m\"/></defs>\n  <rect x=\"10\" y=\"10\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>\n  <line x1=\"30\" y1=\"15\" x2=\"45\" y2=\"50\" data-id=\"s3\"/>\n  <circle cx=\"50\" cy=\"50\" r=\"5\" data-id=\"s2\"/>\n</svg>\n"
        );
        assert!(d.undo().unwrap());
        assert!(d.shape("s4").is_some());

        // Adjacent members need no reorder, and inline members wrap inline.
        let mut inline = Drawing::open("<svg viewBox=\"0 0 9 9\"><rect width=\"1\" height=\"1\" data-id=\"a\"/><rect width=\"1\" height=\"1\" data-id=\"b\"/></svg>").unwrap();
        inline.group(&["a", "b"]).unwrap();
        assert_eq!(
            inline.source(),
            "<svg viewBox=\"0 0 9 9\"><g data-id=\"s1\"><rect width=\"1\" height=\"1\" data-id=\"a\"/><rect width=\"1\" height=\"1\" data-id=\"b\"/></g></svg>"
        );
        inline.ungroup("s1").unwrap();
        assert_eq!(
            inline.source(),
            "<svg viewBox=\"0 0 9 9\"><rect width=\"1\" height=\"1\" data-id=\"a\"/><rect width=\"1\" height=\"1\" data-id=\"b\"/></svg>"
        );
    }

    #[test]
    fn group_refuses_what_is_not_siblings_and_ungroup_what_is_not_a_group() {
        let mut d = Drawing::open(GROUPED).unwrap();
        assert!(matches!(d.group(&["s1", "s2"]), Err(Error::NotSiblings)));
        assert!(matches!(d.group(&["s2", "zz"]), Err(Error::NoSuchShape(_))));
        assert!(matches!(
            d.ungroup("s2"),
            Err(Error::Unsupported {
                gesture: "ungroup",
                kind: ShapeKind::Rect
            })
        ));
        // Members of a group can be grouped again, inside it.
        let inner = d.group(&["s2", "s3"]).unwrap();
        assert_eq!(d.shape(&inner).unwrap().group.as_deref(), Some("s1"));
        assert_eq!(d.outermost("s3").and_then(|s| s.id.as_deref()), Some("s1"));
        assert_eq!(
            d.source(),
            "<svg viewBox=\"0 0 100 100\" data-diaryx-drawing=\"1\">\n  <g data-id=\"s1\" transform=\"translate(10 0)\">\n    <g data-id=\"s4\">\n      <rect x=\"0\" y=\"0\" width=\"10\" height=\"10\" data-id=\"s2\"/>\n      <path d=\"M20 0 h10\" data-id=\"s3\"/>\n    </g>\n  </g>\n</svg>\n"
        );
        d.ungroup("s4").unwrap();
        assert_eq!(d.source(), GROUPED);
    }

    #[test]
    fn ungrouping_a_transformed_group_pushes_the_transform_down() {
        let mut d = Drawing::open(GROUPED).unwrap();
        let before = (d.bounds("s2"), d.bounds("s3"));
        d.ungroup("s1").unwrap();
        assert_eq!(
            d.source(),
            "<svg viewBox=\"0 0 100 100\" data-diaryx-drawing=\"1\">\n  <rect x=\"10\" y=\"0\" width=\"10\" height=\"10\" data-id=\"s2\"/>\n  <path d=\"M30 0 L40 0\" data-id=\"s3\"/>\n</svg>\n"
        );
        assert_eq!((d.bounds("s2"), d.bounds("s3")), before, "nothing moved");
        assert!(d.undo().unwrap(), "three edits, one step");
        assert_eq!(d.source(), GROUPED);
        assert!(!d.undo().unwrap());
    }

    #[test]
    fn move_all_and_delete_all_are_one_step_each() {
        let mut d = Drawing::open(SCENE).unwrap();
        d.move_all(&["s1", "s2"], 1.0, 1.0).unwrap();
        assert_eq!(d.shape("s1").unwrap().attr("x"), Some("11"));
        assert_eq!(d.shape("s2").unwrap().attr("cx"), Some("51"));
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), SCENE);
        d.delete_all(&["s1", "s3"]).unwrap();
        assert_eq!(ids(&d), ["s2"]);
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), SCENE);
        assert!(matches!(d.delete_all(&["zz"]), Err(Error::NoSuchShape(_))));
    }

    /// A measurer that says every label is 40 wide and 10 tall, its box
    /// 8 above the baseline — at the `x` and `y` the label carries.
    struct Fixed;

    impl Measure for Fixed {
        fn measure(&self, svg: &str) -> Option<Bounds> {
            let attr = |name: &str| -> f64 {
                let at = svg.find(&format!(" {name}=\"")).unwrap() + name.len() + 3;
                svg[at..].split('"').next().unwrap().parse().unwrap()
            };
            Some(Bounds {
                x: attr("x"),
                y: attr("y") - 8.0,
                width: 40.0,
                height: 10.0,
            })
        }
    }

    const LABELLED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" font-family=\"serif\" data-diaryx-drawing=\"1\">\n  <style>text { fill: red }</style>\n  <g transform=\"translate(100 0)\" class=\"k\" data-id=\"g\">\n    <text x=\"10\" y=\"50\" transform=\"scale(2)\" data-id=\"t\">Hi <tspan>there</tspan></text>\n  </g>\n</svg>\n";

    #[test]
    fn a_label_is_measured_by_the_host_where_it_is() {
        let mut d = Drawing::open(LABELLED).unwrap();
        assert_eq!(d.shape("t").unwrap().text.as_deref(), Some("Hi there"));
        // Without a measurer: the nominal box, through the chain.
        let nominal = d.bounds("t").unwrap();
        assert_eq!((nominal.x, nominal.y, nominal.height), (120.0, 80.8, 24.0));
        assert!((nominal.width - 115.2).abs() < 1e-9, "{nominal:?}");

        d.set_measure(Box::new(Fixed));
        let b = d.bounds("t").unwrap();
        // (10, 50) + (0, -8), 40 by 10, scaled by 2, then across by 100.
        assert_eq!(
            b,
            Bounds {
                x: 120.0,
                y: 84.0,
                width: 80.0,
                height: 20.0
            }
        );
        assert_eq!(d.hit(150.0, 90.0, 0.0).unwrap().id.as_deref(), Some("t"));
        assert!(d.hit(150.0, 110.0, 0.0).is_none());

        // Moving keeps the measured box.
        d.move_by("t", 10.0, 0.0).unwrap();
        assert_eq!(d.shape("t").unwrap().attr("x"), Some("15"));
        assert_eq!(d.bounds("t").unwrap().x, 130.0);
        // Resizing a label moves its anchor to where the box went.
        d.resize(
            "t",
            Bounds {
                x: 120.0,
                y: 84.0,
                width: 1.0,
                height: 1.0,
            },
        )
        .unwrap();
        assert_eq!(d.shape("t").unwrap().attr("x"), Some("10"));
    }

    #[test]
    fn the_measurer_is_asked_about_the_label_where_it_is_under_its_styles() {
        use std::sync::{Arc, Mutex};
        /// Remembers what it was asked and measures nothing.
        struct Spy(Arc<Mutex<Vec<String>>>);
        impl Measure for Spy {
            fn measure(&self, svg: &str) -> Option<Bounds> {
                self.0.lock().unwrap().push(svg.to_string());
                None
            }
        }
        let mut d = Drawing::open(LABELLED).unwrap();
        let asked = Arc::new(Mutex::new(Vec::new()));
        d.set_measure(Box::new(Spy(asked.clone())));
        d.bounds("t");
        d.bounds("t");
        d.move_by("t", 1.0, 1.0).unwrap();
        d.bounds("t");
        let seen: Vec<String> = asked.lock().unwrap().clone();
        assert_eq!(
            seen,
            [
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\" font-family=\"serif\" data-diaryx-drawing=\"1\"><style>text { fill: red }</style><g class=\"k\" data-id=\"g\"><text x=\"10\" y=\"50\" data-id=\"t\">Hi <tspan>there</tspan></text></g></svg>",
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\" font-family=\"serif\" data-diaryx-drawing=\"1\"><style>text { fill: red }</style><g class=\"k\" data-id=\"g\"><text x=\"10.5\" y=\"50.5\" data-id=\"t\">Hi <tspan>there</tspan></text></g></svg>"
            ],
            "asked once per label as it stands"
        );
    }

    /// The end is on the rim of the circle of radius 10 at `centre`.
    fn on_rim(line: &Shape, end: End, centre: (f64, f64)) {
        let (x, y) = Connector::of(line).unwrap().end(end);
        let r = ((x - centre.0).powi(2) + (y - centre.1).powi(2)).sqrt();
        assert!((r - 10.0).abs() < 0.05, "on the rim of {centre:?}: {x} {y}");
    }

    const WIRED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <rect x=\"10\" y=\"10\" width=\"20\" height=\"20\" data-id=\"s1\"/>\n  <circle cx=\"100\" cy=\"20\" r=\"10\" data-id=\"s2\"/>\n  <line x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\" data-id=\"s3\"/>\n</svg>\n";

    #[test]
    fn a_bound_arrow_sits_on_its_targets_edges_and_follows_them() {
        let mut d = Drawing::open(WIRED).unwrap();
        d.bind("s3", End::From, Some("s1")).unwrap();
        d.bind("s3", End::To, Some("s2")).unwrap();
        let line = d.shape("s3").unwrap();
        // Centres (20, 20) and (100, 20): the rect's right edge, the
        // circle's left.
        assert_eq!(
            line.attrs,
            [
                ("x1", Some("30".into())),
                ("y1", Some("20".into())),
                ("x2", Some("90".into())),
                ("y2", Some("20".into())),
                ("data-id", Some("s3".into())),
                ("data-from", Some("s1".into())),
                ("data-to", Some("s2".into())),
            ]
            .map(|(k, v): (&str, Option<String>)| (k.to_string(), v))
        );
        assert_eq!(d.end_point("s3", End::To), Some((90.0, 20.0)));

        // The circle moves down: the line's end follows around its rim,
        // in the move's one step, and the start leaves the rect lower.
        d.move_by("s2", 0.0, 60.0).unwrap();
        let line = d.shape("s3").unwrap();
        assert_eq!(
            (line.attr("x1"), line.attr("y1")),
            (Some("30"), Some("27.5"))
        );
        on_rim(line, End::To, (100.0, 80.0));
        assert!(d.undo().unwrap());
        assert_eq!(d.shape("s3").unwrap().attr("y1"), Some("20"));
        assert_eq!(d.shape("s2").unwrap().attr("cy"), Some("20"));

        // Dragging the arrow itself moves nothing bound.
        d.move_by("s3", 5.0, 5.0).unwrap();
        assert_eq!(d.shape("s3").unwrap().attr("x1"), Some("30"));
        assert!(d.undo().unwrap());

        // Deleting a target takes the binding off, in the delete's step.
        d.delete("s2").unwrap();
        let line = d.shape("s3").unwrap();
        assert_eq!(line.attr("data-to"), None);
        assert_eq!(line.attr("data-from"), Some("s1"));
        assert_eq!(line.attr("x2"), Some("90"), "the end stays where it was");
        assert!(d.undo().unwrap());
        assert_eq!(d.shape("s3").unwrap().attr("data-to"), Some("s2"));

        // Unbinding leaves the end put.
        d.bind("s3", End::To, None).unwrap();
        assert_eq!(d.shape("s3").unwrap().attr("data-to"), None);
        d.move_by("s2", 0.0, 60.0).unwrap();
        assert_eq!(d.shape("s3").unwrap().attr("x2"), Some("90"));

        assert!(matches!(
            d.bind("s1", End::To, Some("s2")),
            Err(Error::Unsupported {
                gesture: "bind",
                ..
            })
        ));
        assert!(matches!(
            d.bind("s3", End::To, Some("s3")),
            Err(Error::NoSuchShape(_))
        ));
    }

    #[test]
    fn dropping_an_end_on_a_shape_binds_it_and_off_one_unbinds() {
        let mut d = Drawing::open(WIRED).unwrap();
        assert_eq!(
            d.drop_end("s3", End::To, 95.0, 20.0, 2.0)
                .unwrap()
                .as_deref(),
            Some("s2")
        );
        let line = d.shape("s3").unwrap();
        assert_eq!(line.attr("data-to"), Some("s2"));
        // From (0, 0) toward the circle's centre, the end lands on the rim.
        on_rim(line, End::To, (100.0, 20.0));
        let (x2, y2) = (line.number("x2").unwrap(), line.number("y2").unwrap());
        assert!(x2 < 100.0 && y2 < 20.0, "facing the other end: {x2} {y2}");
        assert!(d.undo().unwrap(), "one step");
        assert_eq!(d.source(), WIRED);

        d.drop_end("s3", End::To, 95.0, 20.0, 2.0).unwrap();
        assert_eq!(d.drop_end("s3", End::To, 150.0, 50.0, 2.0).unwrap(), None);
        let line = d.shape("s3").unwrap();
        assert_eq!(line.attr("data-to"), None);
        assert_eq!(
            (line.attr("x2"), line.attr("y2")),
            (Some("150"), Some("50"))
        );
        // An end dropped on the arrow's own stroke binds to nothing.
        assert_eq!(d.drop_end("s3", End::From, 75.0, 25.0, 2.0).unwrap(), None);
    }

    #[test]
    fn an_arrow_bound_into_a_group_follows_the_group_and_a_deleted_group() {
        let mut d = Drawing::open(WIRED).unwrap();
        let g = d.group(&["s1", "s2"]).unwrap();
        d.bind("s3", End::To, Some("s2")).unwrap();
        on_rim(d.shape("s3").unwrap(), End::To, (100.0, 20.0));
        d.move_by(&g, 0.0, 10.0).unwrap();
        on_rim(d.shape("s3").unwrap(), End::To, (100.0, 30.0));
        // Bound to the group, an end leaves the last of its members'
        // outlines on the way out from the group's centre, (60, 30).
        d.drop_end("s3", End::To, 180.0, 30.0, 0.0).unwrap();
        d.bind("s3", End::From, Some(&g)).unwrap();
        let line = d.shape("s3").unwrap();
        assert_eq!(line.attr("data-from"), Some(&*g));
        assert_eq!(
            (line.attr("x1"), line.attr("y1")),
            (Some("110"), Some("30"))
        );
        d.ungroup(&g).unwrap();
        let line = d.shape("s3").unwrap();
        assert_eq!(line.attr("data-from"), None, "nothing to point at");
        assert_eq!(line.attr("x1"), Some("110"));
        assert!(d.undo().unwrap());
        assert_eq!(d.shape("s3").unwrap().attr("data-from"), Some(&*g));
    }

    /// A drawing whose `<style>` predates the bend: `line` is stroked and
    /// nothing says what a `path` is. The first bend widens the rules to
    /// the template's spelling — in the same step — so the path that the
    /// arrow becomes is an outline with its head, not a filled silhouette.
    #[test]
    fn a_bend_in_a_drawing_styled_before_bends_widens_its_line_rules_to_paths() {
        const OLD: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 640 400\" width=\"640\" height=\"400\" data-diaryx-drawing=\"1\">\n  <defs>\n    <marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto-start-reverse\">\n      <path d=\"M 0 0 L 10 5 L 0 10 z\"/>\n    </marker>\n  </defs>\n  <style>\n    rect, ellipse, polygon { fill: none; stroke: #222; stroke-width: 2 }\n    line { stroke: #222; stroke-width: 2 }\n    line[data-arrow=\"end\"], line[data-arrow=\"both\"] { marker-end: url(#arrow) }\n    line[data-arrow=\"start\"], line[data-arrow=\"both\"] { marker-start: url(#arrow) }\n    path[data-ink] { fill: #222; stroke: none }\n    text { font: 16px sans-serif }\n  </style>\n  <line x1=\"200\" y1=\"100\" x2=\"400\" y2=\"100\" data-arrow=\"end\" data-id=\"s1\"/>\n</svg>\n";
        let mut d = Drawing::open(OLD).unwrap();
        d.bend("s1", Some((300.0, 200.0))).unwrap();
        assert!(
            d.source().contains("    line, path { fill: none; stroke: #222; stroke-width: 2 }\n    line[data-arrow=\"end\"], line[data-arrow=\"both\"], path[data-arrow=\"end\"], path[data-arrow=\"both\"] { marker-end: url(#arrow) }\n    line[data-arrow=\"start\"], line[data-arrow=\"both\"], path[data-arrow=\"start\"], path[data-arrow=\"both\"] { marker-start: url(#arrow) }\n    marker path { fill: #222; stroke: none }\n    path[data-ink] { fill: #222; stroke: none }\n"),
            "{}",
            d.source()
        );
        assert!(
            d.source().contains(
                "<path d=\"M200 100 Q300 300 400 100\" data-arrow=\"end\" data-id=\"s1\"/>"
            )
        );
        assert!(d.check().is_empty(), "{:?}", d.check());
        assert!(d.undo().unwrap(), "one step");
        assert_eq!(d.source(), OLD, "the style comes back with the line");
        assert!(d.redo().unwrap());

        // Bent again, there is nothing left to widen; and the template
        // itself, which already styles a path, is never touched.
        let widened = d.source().to_string();
        d.bend("s1", Some((300.0, 150.0))).unwrap();
        assert_eq!(
            d.source().matches("line, path").count(),
            1,
            "{}",
            d.source()
        );
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), widened);
        let mut fresh = Drawing::fresh();
        let id = fresh
            .add_arrow(200.0, 100.0, 400.0, 100.0, Heads::End)
            .unwrap();
        let style_before = fresh.source()
            [fresh.source().find("<style>").unwrap()..fresh.source().find("</style>").unwrap()]
            .to_string();
        fresh.bend(&id, Some((300.0, 200.0))).unwrap();
        assert!(fresh.source().contains(&style_before));

        // Drawn by usvg, the bent arrow is a stroke with a head at its
        // end, not a filled crescent.
        #[cfg(feature = "usvg")]
        assert_eq!(painted(d.source()), vec![Paint::Stroked, Paint::Filled]);
    }

    /// How usvg paints each path of a drawing, in order — the shape,
    /// then any marker instanced on it.
    #[cfg(feature = "usvg")]
    #[derive(Debug, PartialEq)]
    enum Paint {
        Filled,
        Stroked,
        Both,
        Neither,
    }

    #[cfg(feature = "usvg")]
    fn painted(svg: &str) -> Vec<Paint> {
        fn walk(g: &usvg::Group, out: &mut Vec<Paint>) {
            for node in g.children() {
                match node {
                    usvg::Node::Path(p) => {
                        out.push(match (p.fill().is_some(), p.stroke().is_some()) {
                            (true, true) => Paint::Both,
                            (true, false) => Paint::Filled,
                            (false, true) => Paint::Stroked,
                            (false, false) => Paint::Neither,
                        })
                    }
                    usvg::Node::Group(g) => walk(g, out),
                    _ => {}
                }
            }
        }
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
        let mut out = Vec::new();
        walk(tree.root(), &mut out);
        out
    }

    /// The template draws an arrow as a stroke with a filled head,
    /// straight or bent, and an ink stroke as a fill.
    #[cfg(feature = "usvg")]
    #[test]
    fn the_template_strokes_a_connector_and_fills_its_head() {
        let mut d = Drawing::fresh();
        let a = d.add_arrow(20.0, 20.0, 120.0, 20.0, Heads::End).unwrap();
        assert_eq!(painted(d.source()), vec![Paint::Stroked, Paint::Filled]);
        d.bend(&a, Some((70.0, 60.0))).unwrap();
        assert_eq!(painted(d.source()), vec![Paint::Stroked, Paint::Filled]);
        d.add_ink(&[(10.0, 100.0), (60.0, 110.0)], &[4.0], Nib::Monoline)
            .unwrap();
        assert_eq!(
            painted(d.source()),
            vec![Paint::Stroked, Paint::Filled, Paint::Filled]
        );
    }

    /// The dash rules the template carries, as `style_word` adds them to
    /// a drawing that has none.
    const DASH_RULES_ADDED: &str = "    [data-dash=\"dashed\"] { stroke-dasharray: 8 6 }\n    [data-dash=\"dotted\"] { stroke-dasharray: 1 5; stroke-linecap: round }\n";

    #[test]
    fn set_dash_writes_the_word_and_brings_its_rules_to_an_older_stylesheet() {
        // A drawing from before the word: a `<style>` with no rule for it.
        let old = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <style>\n    rect { fill: none; stroke: #222 }\n  </style>\n  <rect x=\"10\" y=\"10\" width=\"20\" height=\"20\" data-id=\"s1\"/>\n  <text x=\"5\" y=\"50\" data-id=\"s2\">hi</text>\n</svg>\n".to_string();
        let mut d = Drawing::open(&old).unwrap();
        d.set_dash("s1", Some(Dash::Dashed)).unwrap();
        assert_eq!(d.shape("s1").unwrap().dash(), Some(Dash::Dashed));
        assert!(
            d.source().contains(&format!(
                "    rect {{ fill: none; stroke: #222 }}\n{DASH_RULES_ADDED}  </style>"
            )),
            "{}",
            d.source()
        );
        assert!(d.source().contains("data-id=\"s1\" data-dash=\"dashed\"/>"));
        assert_eq!(d.check(), []);
        // The word and the rules are one step.
        d.undo().unwrap();
        assert_eq!(d.source(), old);
        d.redo().unwrap();
        // A second word finds the rules there; solid takes the word off
        // and leaves them.
        d.set_dash("s1", Some(Dash::Dotted)).unwrap();
        assert_eq!(d.shape("s1").unwrap().dash(), Some(Dash::Dotted));
        assert_eq!(d.source().matches("[data-dash=\"dashed\"]").count(), 1);
        d.set_dash("s1", None).unwrap();
        assert_eq!(d.shape("s1").unwrap().dash(), None);
        assert!(d.source().contains("data-id=\"s1\"/>"), "{}", d.source());
        assert!(d.source().contains(DASH_RULES_ADDED), "the rules stay");
        // Not a stroke: refused; a selection skips it.
        assert!(matches!(
            d.set_dash("s2", Some(Dash::Dashed)),
            Err(Error::Unsupported {
                gesture: "set_dash",
                ..
            })
        ));
        d.set_dash_all(&["s2", "s1"], Some(Dash::Dashed)).unwrap();
        assert_eq!(d.shape("s1").unwrap().dash(), Some(Dash::Dashed));
        assert_eq!(d.shape("s2").unwrap().dash(), None);

        // A stylesheet that says what the word means is left alone.
        let themed = old.replace("rect {", "[data-dash] { stroke-dasharray: 2 }\n    rect {");
        let mut d = Drawing::open(&themed).unwrap();
        d.set_dash("s1", Some(Dash::Dashed)).unwrap();
        assert!(!d.source().contains("8 6"), "{}", d.source());
    }

    #[test]
    fn the_pen_writes_its_words_into_a_shape_as_it_is_added() {
        let mut d = Drawing::fresh();
        assert_eq!(d.pen(), Pen::default());
        d.set_pen(Pen {
            dash: Some(Dash::Dotted),
        });
        let r = d
            .add_rect(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            })
            .unwrap();
        let l = d.add_line(0.0, 0.0, 10.0, 10.0).unwrap();
        let t = d.add_text(0.0, 0.0, "hi").unwrap();
        let i = d
            .add_ink(&[(0.0, 0.0), (5.0, 5.0)], &[2.0], Nib::Monoline)
            .unwrap();
        let n = d
            .add_note(
                Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 60.0,
                    height: 40.0,
                },
                "note",
            )
            .unwrap();
        for id in [&r, &l] {
            assert_eq!(d.shape(id).unwrap().dash(), Some(Dash::Dotted), "{id}");
        }
        for id in [&t, &i, &n] {
            assert_eq!(d.shape(id).unwrap().attr("data-dash"), None, "{id}");
        }
        let frame = d.note(&n).unwrap().frame;
        assert_eq!(
            d.shape(&frame).unwrap().dash(),
            Some(Dash::Dotted),
            "a note's frame"
        );
        assert_eq!(d.check(), []);
        // The template has the rules; the shape's step is one step.
        assert_eq!(d.source().matches("[data-dash=\"dotted\"]").count(), 1);
        for _ in 0..5 {
            assert!(d.undo().unwrap());
        }
        assert_eq!(d.source(), crate::profile::TEMPLATE);
        // A note's dash is its frame's, either way.
        d.set_pen(Pen::default());
        let n = d
            .add_note(
                Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 60.0,
                    height: 40.0,
                },
                "note",
            )
            .unwrap();
        d.set_dash(&n, Some(Dash::Dashed)).unwrap();
        let frame = d.note(&n).unwrap().frame;
        assert_eq!(d.shape(&frame).unwrap().dash(), Some(Dash::Dashed));
        assert_eq!(d.shape(&n).unwrap().attr("data-dash"), None);
    }

    #[test]
    fn set_heads_writes_the_word_takes_it_off_and_replaces_a_spelled_marker() {
        let mut d = Drawing::open(WIRED).unwrap();
        // A plain line takes a head at each end, appended after what it has.
        d.set_heads("s3", Some(Heads::Both)).unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), Some(Heads::Both));
        assert!(
            d.source().contains(
                "<line x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\" data-id=\"s3\" data-arrow=\"both\"/>"
            ),
            "{}",
            d.source()
        );
        // The same word again writes nothing, so undo goes past it.
        d.set_heads("s3", Some(Heads::Both)).unwrap();
        // Off again is a line, and one undo step brings the word back.
        d.set_heads("s3", None).unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), None);
        assert_eq!(d.source(), WIRED);
        d.undo().unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), Some(Heads::Both));
        d.undo().unwrap();
        assert_eq!(d.source(), WIRED, "the no-op was not a step");
        // Not a connector: refused.
        assert!(matches!(
            d.set_heads("s1", Some(Heads::End)),
            Err(Error::Unsupported {
                gesture: "set_heads",
                ..
            })
        ));

        // A file that spells its own marker: the word replaces it.
        let spelled = WIRED.replace(
            "data-id=\"s3\"",
            "marker-end=\"url(#arrow)\" data-id=\"s3\"",
        );
        let mut d = Drawing::open(&spelled).unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), Some(Heads::End));
        d.set_heads("s3", Some(Heads::Start)).unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), Some(Heads::Start));
        assert!(!d.source().contains("marker-end"), "{}", d.source());
        // Taking the heads off a spelled arrow takes the marker off too.
        let mut d = Drawing::open(&spelled).unwrap();
        d.set_heads("s3", None).unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), None);
        assert_eq!(d.source(), WIRED);
    }

    #[test]
    fn set_heads_all_takes_the_connectors_of_a_selection_in_one_step() {
        let mut d = Drawing::open(WIRED).unwrap();
        let s4 = d.add_arrow(0.0, 50.0, 50.0, 50.0, Heads::End).unwrap();
        // The rect and circle are skipped, both lines take the word, and
        // one undo step brings both back.
        d.set_heads_all(&["s1", "s3", "s2", &s4], Some(Heads::Both))
            .unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), Some(Heads::Both));
        assert_eq!(d.shape(&s4).unwrap().heads(), Some(Heads::Both));
        d.undo().unwrap();
        assert_eq!(d.shape("s3").unwrap().heads(), None);
        assert_eq!(d.shape(&s4).unwrap().heads(), Some(Heads::End));
        assert!(matches!(
            d.set_heads_all(&["s3", "nope"], None),
            Err(Error::NoSuchShape(_))
        ));
    }

    #[test]
    fn a_bend_makes_a_line_a_path_and_back_and_a_bound_end_faces_the_bend() {
        let mut d = Drawing::open(WIRED).unwrap();
        d.bind("s3", End::From, Some("s1")).unwrap();
        d.bind("s3", End::To, Some("s2")).unwrap();
        // Straight, from (30, 20) to (90, 20); its midpoint is the handle.
        let c = d.connector("s3").unwrap();
        assert_eq!(
            (c.from, c.to, c.control),
            ((30.0, 20.0), (90.0, 20.0), None)
        );
        assert_eq!(c.midpoint(), (60.0, 20.0));

        // Pulled down to pass through (60, 50): the line becomes a path
        // with `d` where `x1` was, the binding attributes kept, and each
        // bound end re-settled to leave its shape toward the control
        // point.
        d.bend("s3", Some((60.0, 50.0))).unwrap();
        let shape = d.shape("s3").unwrap();
        assert_eq!(shape.kind, ShapeKind::Path);
        assert!(
            d.source().contains("\n  <path d=\"M"),
            "d where x1 was: {}",
            d.source()
        );
        assert!(
            d.source()
                .contains("data-id=\"s3\" data-from=\"s1\" data-to=\"s2\"/>")
        );
        let c = d.connector("s3").unwrap();
        let (mx, my) = c.midpoint();
        assert!(
            (mx - 60.0).abs() < 0.01 && (my - 50.0).abs() < 0.01,
            "{c:?}"
        );
        // From the rect's centre toward a control point below and right,
        // the start leaves the rect's bottom edge.
        assert!(
            c.from.1 == 30.0 && c.from.0 > 20.0 && c.from.0 < 30.0,
            "{c:?}"
        );
        on_rim(shape, End::To, (100.0, 20.0));
        assert!(
            c.to.1 > 20.0,
            "leaves the rim below its leftmost point: {c:?}"
        );
        assert!(d.check().is_empty(), "{:?}", d.check());
        // One step.
        assert!(d.undo().unwrap());
        assert_eq!(d.shape("s3").unwrap().kind, ShapeKind::Line);
        assert!(d.redo().unwrap());

        // The circle moves: the bent arrow's end follows, in the path's
        // `d`, and the bend is kept.
        let control = d.connector("s3").unwrap().control;
        d.move_by("s2", 0.0, 60.0).unwrap();
        let c = d.connector("s3").unwrap();
        on_rim(d.shape("s3").unwrap(), End::To, (100.0, 80.0));
        assert_eq!(c.control, control);
        assert!(d.undo().unwrap());

        // Moving the arrow itself bakes into the `d` and keeps it a
        // connector; the bytes it writes are the bytes a bend writes.
        d.bind("s3", End::From, None).unwrap();
        d.bind("s3", End::To, None).unwrap();
        let before = d.shape("s3").unwrap().attr("d").unwrap().to_string();
        let unbound = d.connector("s3").unwrap();
        d.move_by("s3", 10.0, 0.0).unwrap();
        let moved = d.connector("s3").unwrap();
        assert_eq!(moved.from, (unbound.from.0 + 10.0, unbound.from.1));
        assert!(d.shape("s3").unwrap().attr("transform").is_none());
        d.move_by("s3", -10.0, 0.0).unwrap();
        assert_eq!(d.shape("s3").unwrap().attr("d"), Some(before.as_str()));

        // An end dragged keeps the control point where it was.
        d.drop_end("s3", End::To, 150.0, 90.0, 0.0).unwrap();
        let c = d.connector("s3").unwrap();
        assert_eq!((c.to, c.control), ((150.0, 90.0), unbound.control));

        // Straightened, it is a `<line>` again, its ends where they were.
        assert!(d.bend("s3", None).unwrap());
        let shape = d.shape("s3").unwrap();
        assert_eq!(shape.kind, ShapeKind::Line);
        assert_eq!(
            (shape.attr("x2"), shape.attr("y2")),
            (Some("150"), Some("90"))
        );
        assert!(d.source().contains("<line x1=\""));
        assert_eq!(d.connector("s3").unwrap().control, None);
        // Straightening a line is nothing: no step, and it says so.
        let before = d.source().to_string();
        assert!(!d.bend("s3", None).unwrap());
        assert_eq!(d.source(), before);
        assert!(d.check().is_empty());

        assert!(matches!(
            d.bend("s1", Some((0.0, 0.0))),
            Err(Error::Unsupported {
                gesture: "bend",
                ..
            })
        ));
    }

    #[test]
    fn a_hand_written_one_segment_path_is_a_connector_and_ink_is_not() {
        let src = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <circle cx=\"100\" cy=\"20\" r=\"10\" data-id=\"s2\"/>\n  <path d=\"M 0 0 q 50 80, 100 0\" data-arrow=\"end\" data-id=\"p\"/>\n  <path d=\"M0 0 L1 1\" data-ink=\"monoline\" data-centreline=\"M0 0 L1 1\" data-widths=\"2\" data-id=\"i\"/>\n</svg>\n";
        let mut d = Drawing::open(src).unwrap();
        assert_eq!(
            d.connector("p"),
            Some(Connector {
                from: (0.0, 0.0),
                to: (100.0, 0.0),
                control: Some((50.0, 80.0))
            })
        );
        assert_eq!(d.shape("p").unwrap().heads(), Some(Heads::End));
        assert_eq!(d.connector("i"), None);
        assert!(matches!(
            d.bend("i", None),
            Err(Error::Unsupported {
                gesture: "bend",
                ..
            })
        ));
        // Bound, the `d` is rewritten in the profile's spelling.
        d.bind("p", End::To, Some("s2")).unwrap();
        let p = d.shape("p").unwrap();
        assert!(
            p.attr("d").unwrap().starts_with("M0 0 Q50 80 "),
            "{:?}",
            p.attr("d")
        );
        on_rim(p, End::To, (100.0, 20.0));
    }

    #[test]
    fn set_text_replaces_the_characters_and_nothing_else() {
        let mut d = Drawing::open(LABELLED).unwrap();
        d.set_text("t", "Ho & <hum>").unwrap();
        assert!(d.source().contains(
            "<text x=\"10\" y=\"50\" transform=\"scale(2)\" data-id=\"t\">Ho &amp; &lt;hum&gt;</text>"
        ));
        assert_eq!(d.shape("t").unwrap().text.as_deref(), Some("Ho & <hum>"));
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), LABELLED);
        assert!(matches!(
            d.set_text("g", "x"),
            Err(Error::Unsupported {
                gesture: "set text",
                ..
            })
        ));

        let mut d = Drawing::open(
            "<svg viewBox=\"0 0 9 9\">\n  <text x=\"1\" y=\"2\" data-id=\"t\"/>\n</svg>\n",
        )
        .unwrap();
        d.set_text("t", "Hi").unwrap();
        assert_eq!(
            d.source(),
            "<svg viewBox=\"-15 -23.6 46.4 44\">\n  <text x=\"1\" y=\"2\" data-id=\"t\">Hi</text>\n</svg>\n"
        );
        d.set_text("t", "").unwrap();
        assert_eq!(d.shape("t").unwrap().text, None);
    }

    #[test]
    fn a_label_with_a_width_flows_into_tspan_lines() {
        // Nominal widths: 12 × 0.6 = 7.2 per character.
        let mut d = Drawing::open(
            "<svg viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <text x=\"10\" y=\"20\" data-id=\"t\">one</text>\n</svg>\n",
        )
        .unwrap();
        d.set_width("t", Some(80.0)).unwrap();
        d.set_text(
            "t",
            "the quick brown fox jumps over extraordinarily lazy dogs",
        )
        .unwrap();
        // 80 wide takes eleven characters: "the quick" (9), "brown fox"
        // (9), "jumps over" (10), then a word too long for any line.
        assert_eq!(
            d.source(),
            "<svg viewBox=\"-6 -5.6 112 116\" data-diaryx-drawing=\"1\">\n  <text x=\"10\" y=\"20\" data-id=\"t\" data-width=\"80\"><tspan x=\"10\">the quick</tspan><tspan x=\"10\" dy=\"1.2em\">brown fox</tspan><tspan x=\"10\" dy=\"1.2em\">jumps over</tspan><tspan x=\"10\" dy=\"1.2em\">extraordinarily</tspan><tspan x=\"10\" dy=\"1.2em\">lazy dogs</tspan></text>\n</svg>\n"
        );
        assert_eq!(
            d.shape("t").unwrap().text.as_deref(),
            Some("the quick brown fox jumps over extraordinarily lazy dogs"),
            "the words come back as one text"
        );
        // The nominal box with no measurer: 56 characters at 7.2 is six
        // lines' worth of 80 (the greedy flow, above, packs it into five).
        let b = d.bounds("t").unwrap();
        assert_eq!((b.width, b.height), (80.0, 12.0 + 5.0 * 1.2 * 12.0));
        assert_eq!(d.check(), []);

        // Moved across, every line moves: the `<tspan>`s carry the `x`.
        d.move_by("t", 5.0, 0.0).unwrap();
        assert!(d.source().contains(
            "<text x=\"15\" y=\"20\" data-id=\"t\" data-width=\"80\"><tspan x=\"15\">the quick</tspan><tspan x=\"15\" dy=\"1.2em\">brown fox</tspan>"
        ));
        assert_eq!(d.bounds("t").unwrap().x, 15.0);
        assert!(d.undo().unwrap(), "one step");
        assert_eq!(d.bounds("t").unwrap().x, 10.0);

        // Narrower by the handle: re-flowed, the box's corner kept.
        d.resize(
            "t",
            Bounds {
                x: b.x,
                y: b.y,
                width: 40.0,
                height: b.height,
            },
        )
        .unwrap();
        assert_eq!(d.shape("t").unwrap().attr("data-width"), Some("40"));
        assert!(
            d.source()
                .contains("<tspan x=\"10\">the</tspan><tspan x=\"10\" dy=\"1.2em\">quick</tspan>")
        );
        assert!(d.undo().unwrap(), "one step");
        assert_eq!(d.shape("t").unwrap().attr("data-width"), Some("80"));

        // Unwrapped: one run again, one step.
        d.set_width("t", None).unwrap();
        assert!(d.source().contains(
            "<text x=\"10\" y=\"20\" data-id=\"t\">the quick brown fox jumps over extraordinarily lazy dogs</text>"
        ));
        assert!(d.undo().unwrap());
        assert!(d.source().contains("data-width=\"80\"><tspan"));
        assert!(matches!(
            d.set_width("zz", None),
            Err(Error::NoSuchShape(_))
        ));
    }

    /// The flow measured by resvg's layout: lines fit the width, and the
    /// `dy="1.2em"` between them lays out as a line each.
    #[cfg(feature = "usvg")]
    #[test]
    fn a_wrapped_label_lays_out_as_lines_in_usvg() {
        let mut d = Drawing::open(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <text x=\"10\" y=\"20\" font-size=\"10\" data-id=\"t\">one</text>\n</svg>\n",
        )
        .unwrap();
        d.set_measure(Box::new(crate::measure::Usvg));
        let Some(one) = d.bounds("t") else {
            eprintln!("no system fonts: nothing to measure");
            return;
        };
        assert_eq!(d.font_size(Some("t")), 10.0);
        assert_eq!(
            d.font_size(None),
            12.0,
            "usvg's default, with no stylesheet"
        );
        d.set_width("t", Some(60.0)).unwrap();
        d.set_text("t", "the quick brown fox jumps over the lazy dog")
            .unwrap();
        let lines = d.source().matches("<tspan").count();
        assert!(lines >= 3, "{}", d.source());
        let b = d.bounds("t").unwrap();
        assert!(b.width <= 60.5, "every line fits: {b:?}");
        let pitch = (b.height - one.height) / (lines - 1) as f64;
        assert!(
            (pitch - 12.0).abs() < 0.5,
            "1.2em of 10 between lines: {b:?}, {lines} lines"
        );
    }

    #[test]
    fn a_newline_in_a_label_is_a_line_of_its_own_and_reads_back() {
        let mut d = Drawing::open(
            "<svg viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <text x=\"10\" y=\"20\" data-id=\"t\">one</text>\n</svg>\n",
        )
        .unwrap();
        d.set_text("t", "first line\nsecond\n\n  third  ").unwrap();
        assert!(d.source().contains(
            "<text x=\"10\" y=\"20\" data-id=\"t\"><tspan x=\"10\">first line</tspan><tspan x=\"10\" dy=\"1.2em\" data-break=\"hard\">second</tspan><tspan x=\"10\" dy=\"1.2em\" data-break=\"hard\">third</tspan></text>"
        ));
        assert_eq!(
            d.shape("t").unwrap().text.as_deref(),
            Some("first line\nsecond\nthird")
        );
        // Wrapped narrow, a hard break stays hard and a soft one is not.
        d.set_width("t", Some(40.0)).unwrap();
        assert!(d.source().contains(
            "<tspan x=\"10\">first</tspan><tspan x=\"10\" dy=\"1.2em\">line</tspan><tspan x=\"10\" dy=\"1.2em\" data-break=\"hard\">second</tspan>"
        ));
        assert_eq!(
            d.shape("t").unwrap().text.as_deref(),
            Some("first line\nsecond\nthird")
        );
        d.set_width("t", None).unwrap();
        assert_eq!(d.source().matches("<tspan").count(), 3);
        d.set_text("t", "one line").unwrap();
        assert!(
            d.source().contains("data-id=\"t\">one line</text>"),
            "no tspans for one line unwrapped"
        );
        assert_eq!(d.check(), []);
    }

    #[test]
    fn reorder_moves_among_sibling_shapes_and_keeps_the_lines() {
        let mut d = Drawing::open(SCENE).unwrap();
        assert!(d.reorder("s1", Order::Forward).unwrap());
        assert_eq!(ids(&d), ["s2", "s1", "s3"]);
        assert!(d.reorder("s1", Order::ToFront).unwrap());
        assert_eq!(ids(&d), ["s2", "s3", "s1"]);
        assert!(
            !d.reorder("s1", Order::Forward).unwrap(),
            "already at the front"
        );
        assert!(!d.reorder("s1", Order::ToFront).unwrap());
        assert!(d.reorder("s1", Order::ToBack).unwrap());
        assert_eq!(ids(&d), ["s1", "s2", "s3"]);
        // <defs> stayed first: to-back is among shapes.
        assert_eq!(d.source(), SCENE);
        assert!(d.reorder("s3", Order::Backward).unwrap());
        assert_eq!(
            d.source(),
            "<svg viewBox=\"0 0 100 100\" data-diaryx-drawing=\"1\">\n  <defs><marker id=\"m\"/></defs>\n  <rect x=\"10\" y=\"10\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>\n  <line x1=\"30\" y1=\"15\" x2=\"45\" y2=\"50\" data-id=\"s3\"/>\n  <circle cx=\"50\" cy=\"50\" r=\"5\" data-id=\"s2\"/>\n</svg>\n"
        );
        assert!(d.undo().unwrap());
        assert_eq!(d.source(), SCENE);
    }

    #[test]
    fn deleting_a_group_takes_its_members() {
        let mut d = Drawing::open("<svg viewBox=\"0 0 9 9\">\n  <g data-id=\"s2\"><circle cx=\"5\" cy=\"5\" r=\"1\" data-id=\"s3\"/></g>\n</svg>\n").unwrap();
        d.delete("s2").unwrap();
        assert!(d.shapes().is_empty());
    }
}
