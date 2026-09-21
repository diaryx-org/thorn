//! The shape model: which elements the editor treats as shapes, and what it
//! reads off each. Everything is read from twig's flat node list after every
//! edit — the SVG is the model, and this is a view of it, never a copy that
//! could drift.

use twig::{FlatNode, Kind, NodeId};

/// What kind of shape an element is. The set the profile names; an element
/// with any other tag is preserved and not modelled.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Circle,
    Line,
    Polyline,
    Polygon,
    /// A `<path>`: freehand ink, an arrow's shaft, or anything a hand wrote.
    Path,
    /// A `<text>` label.
    Text,
    /// A `<g>`: a group of shapes, itself selectable.
    Group,
    /// An `<image>`: a photograph or a bitmap the drawing is marked up over.
    Image,
}

impl ShapeKind {
    /// The tag that spells this kind.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Circle => "circle",
            Self::Line => "line",
            Self::Polyline => "polyline",
            Self::Polygon => "polygon",
            Self::Path => "path",
            Self::Text => "text",
            Self::Group => "g",
            Self::Image => "image",
        }
    }

    /// The kind an element tag spells, or `None` for a tag that is not a
    /// shape (`defs`, `style`, `title`, a `<marker>` inside `<defs>` …).
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "rect" => Self::Rect,
            "ellipse" => Self::Ellipse,
            "circle" => Self::Circle,
            "line" => Self::Line,
            "polyline" => Self::Polyline,
            "polygon" => Self::Polygon,
            "path" => Self::Path,
            "text" => Self::Text,
            "g" => Self::Group,
            "image" => Self::Image,
            _ => return None,
        })
    }

    /// The attributes that carry this kind's geometry, which the profile
    /// requires in its number format and which move and resize rewrite.
    pub fn geometry_attrs(self) -> &'static [&'static str] {
        match self {
            Self::Rect | Self::Image => &["x", "y", "width", "height", "rx", "ry"],
            Self::Ellipse => &["cx", "cy", "rx", "ry"],
            Self::Circle => &["cx", "cy", "r"],
            Self::Line => &["x1", "y1", "x2", "y2"],
            // `data-width` is the width a label wraps to.
            Self::Text => &["x", "y", "data-width"],
            // `points` and `d` are lists, checked as such; a group has none.
            Self::Polyline | Self::Polygon | Self::Path | Self::Group => &[],
        }
    }
}

/// One shape, as read from the tree. Holds the node's identity and its
/// attributes as strings — the numbers a gesture keeps while a drag is in
/// flight belong to the gesture, and are written back here when it lands.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// twig's node, valid until the next edit.
    pub node: NodeId,
    pub kind: ShapeKind,
    /// The `data-id` the profile requires; `None` is a profile finding, not
    /// an editor error, because the file may have been written by hand.
    pub id: Option<String>,
    /// The enclosing group's `data-id`, when the shape is inside a `<g>` that
    /// is itself a shape; `None` for a shape directly under `<svg>`.
    pub group: Option<String>,
    /// Nesting depth below `<svg>`: 0 for a direct child.
    pub depth: usize,
    /// Every attribute, in source order, bare ones as `None`.
    pub attrs: Vec<(String, Option<String>)>,
    /// A `<text>`'s characters — every text node inside it, `<tspan>`s
    /// included, whitespace collapsed as SVG lays it out; a `<tspan>`
    /// carrying `data-break` starts a new line, `\n` here. `None` for
    /// every other kind, and for an empty label.
    pub text: Option<String>,
}

/// Which ends of an arrow have a head: the values `data-arrow` takes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Heads {
    /// A head at `(x2, y2)`: the arrow the tool draws.
    End,
    /// A head at `(x1, y1)`.
    Start,
    Both,
}

impl Heads {
    /// The value `data-arrow` spells this as.
    pub fn value(self) -> &'static str {
        match self {
            Self::End => "end",
            Self::Start => "start",
            Self::Both => "both",
        }
    }

    /// The heads a `data-arrow` value names, or `None` for a value the
    /// profile does not admit.
    pub fn from_value(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "end" => Self::End,
            "start" => Self::Start,
            "both" => Self::Both,
            _ => return None,
        })
    }
}

/// How a stroke is broken: the values `data-dash` takes. Solid is the
/// word's absence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Dash {
    Dashed,
    Dotted,
}

impl Dash {
    /// The value `data-dash` spells this as.
    pub fn value(self) -> &'static str {
        match self {
            Self::Dashed => "dashed",
            Self::Dotted => "dotted",
        }
    }

    /// The dash a `data-dash` value names, or `None` for a value the
    /// profile does not admit.
    pub fn from_value(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "dashed" => Self::Dashed,
            "dotted" => Self::Dotted,
            _ => return None,
        })
    }
}

/// How heavy a stroke is: the values `data-weight` takes. The template's
/// width is the word's absence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Weight {
    Thin,
    Bold,
}

impl Weight {
    /// The value `data-weight` spells this as.
    pub fn value(self) -> &'static str {
        match self {
            Self::Thin => "thin",
            Self::Bold => "bold",
        }
    }

    /// The weight a `data-weight` value names, or `None` for a value the
    /// profile does not admit.
    pub fn from_value(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "thin" => Self::Thin,
            "bold" => Self::Bold,
            _ => return None,
        })
    }
}

/// A colour of the palette: the values `data-color` and `data-fill` take.
/// A name, not a hex, so the drawing's `<style>` can say what red is on a
/// light page and on a dark one (docs/proposals/shape-style.md).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
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

impl Hue {
    /// Every hue, in the order a palette shows them.
    pub const ALL: [Hue; 8] = [
        Hue::Red,
        Hue::Orange,
        Hue::Yellow,
        Hue::Green,
        Hue::Blue,
        Hue::Violet,
        Hue::Pink,
        Hue::Grey,
    ];

    /// The value `data-color` or `data-fill` spells this as.
    pub fn value(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Violet => "violet",
            Self::Pink => "pink",
            Self::Grey => "grey",
        }
    }

    /// The hue a value names, or `None` for one the profile does not admit.
    pub fn from_value(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "red" => Self::Red,
            "orange" => Self::Orange,
            "yellow" => Self::Yellow,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "violet" => Self::Violet,
            "pink" => Self::Pink,
            "grey" => Self::Grey,
            _ => return None,
        })
    }

    /// What the template draws a `data-color` of this hue as: the `color`
    /// a stroke, an ink stroke, a label and an arrowhead take, on a light
    /// page and on a dark one.
    pub fn stroke(self, dark: bool) -> &'static str {
        match (self, dark) {
            (Self::Red, false) => "#c62828",
            (Self::Red, true) => "#ef5350",
            (Self::Orange, false) => "#ef6c00",
            (Self::Orange, true) => "#ffa726",
            (Self::Yellow, false) => "#f9a825",
            (Self::Yellow, true) => "#ffee58",
            (Self::Green, false) => "#2e7d32",
            (Self::Green, true) => "#66bb6a",
            (Self::Blue, false) => "#1565c0",
            (Self::Blue, true) => "#42a5f5",
            (Self::Violet, false) => "#6a1b9a",
            (Self::Violet, true) => "#ab47bc",
            (Self::Pink, false) => "#ad1457",
            (Self::Pink, true) => "#ec407a",
            (Self::Grey, false) => "#757575",
            (Self::Grey, true) => "#9e9e9e",
        }
    }

    /// What the template draws a `data-fill` of this hue as: a tint, on a
    /// light page and on a dark one.
    pub fn tint(self, dark: bool) -> &'static str {
        match (self, dark) {
            (Self::Red, false) => "#ffcdd2",
            (Self::Red, true) => "#4e1c1c",
            (Self::Orange, false) => "#ffe0b2",
            (Self::Orange, true) => "#4e2f0f",
            (Self::Yellow, false) => "#fff9c4",
            (Self::Yellow, true) => "#4a4210",
            (Self::Green, false) => "#c8e6c9",
            (Self::Green, true) => "#1b3d1f",
            (Self::Blue, false) => "#bbdefb",
            (Self::Blue, true) => "#10305a",
            (Self::Violet, false) => "#e1bee7",
            (Self::Violet, true) => "#3a1550",
            (Self::Pink, false) => "#f8bbd0",
            (Self::Pink, true) => "#4a1330",
            (Self::Grey, false) => "#e0e0e0",
            (Self::Grey, true) => "#3a3a3a",
        }
    }
}

impl Shape {
    /// Whether the editor draws this shape as a closed figure — a box, an
    /// ellipse, a polygon — and so whether a `data-fill` means anything on
    /// it.
    pub fn is_closed(&self) -> bool {
        matches!(
            self.kind,
            ShapeKind::Rect | ShapeKind::Ellipse | ShapeKind::Circle | ShapeKind::Polygon
        )
    }

    /// Whether a `data-color` means anything on this shape: every kind
    /// the template draws in `currentColor` — everything but a group,
    /// which is coloured through its members, and an image.
    pub fn takes_color(&self) -> bool {
        !matches!(self.kind, ShapeKind::Group | ShapeKind::Image)
    }

    /// How heavy the stroke is, from `data-weight`; `None` for the
    /// template's width, or a value the profile does not admit.
    pub fn weight(&self) -> Option<Weight> {
        self.attr("data-weight").and_then(Weight::from_value)
    }

    /// The hue of `data-color`; `None` for the drawing's ink, or a value
    /// the profile does not admit (which `check` reports).
    pub fn hue(&self) -> Option<Hue> {
        self.attr("data-color").and_then(Hue::from_value)
    }

    /// The hue of `data-fill`; `None` for no background, or a value the
    /// profile does not admit.
    pub fn fill(&self) -> Option<Hue> {
        self.attr("data-fill").and_then(Hue::from_value)
    }

    /// Whether the editor draws this shape as a stroke — a box, a line, a
    /// connector — and so whether a `data-dash` or a `data-weight` means
    /// anything on it. Ink is a filled outline, a label is glyphs, a group
    /// and an image are neither.
    pub fn is_stroked(&self) -> bool {
        match self.kind {
            ShapeKind::Rect
            | ShapeKind::Ellipse
            | ShapeKind::Circle
            | ShapeKind::Line
            | ShapeKind::Polyline
            | ShapeKind::Polygon => true,
            ShapeKind::Path => self.attr("data-ink").is_none(),
            ShapeKind::Text | ShapeKind::Group | ShapeKind::Image => false,
        }
    }

    /// How the stroke is broken, from `data-dash`; `None` for solid, or a
    /// value the profile does not admit (which `check` reports).
    pub fn dash(&self) -> Option<Dash> {
        self.attr("data-dash").and_then(Dash::from_value)
    }

    /// Whether this is an arrow, and which ends have a head: what
    /// `data-arrow` says (the profile's spelling, which the template's
    /// `<style>` draws), or, failing that, a `marker-end`/`marker-start`
    /// the file spells itself. `None` for a line with no head.
    pub fn heads(&self) -> Option<Heads> {
        if let Some(v) = self.attr("data-arrow") {
            return Heads::from_value(v);
        }
        match (self.attr("marker-start"), self.attr("marker-end")) {
            (Some(_), Some(_)) => Some(Heads::Both),
            (Some(_), None) => Some(Heads::Start),
            (None, Some(_)) => Some(Heads::End),
            (None, None) => None,
        }
    }

    /// The value of an attribute, if present and not bare.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.as_deref())
    }

    /// A geometry attribute as a number, when it is present and parses.
    pub fn number(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(|v| v.trim().parse().ok())
    }
}

/// Read the shapes out of a node list, in document (paint) order, walking
/// into groups. `root` is the `<svg>` element.
pub(crate) fn read(nodes: &[FlatNode], root: NodeId) -> Vec<Shape> {
    let mut shapes = Vec::new();
    walk(nodes, root, 0, None, &mut shapes);
    shapes
}

/// Depth-first in source order. twig's list is post-order, so children are
/// found through `first_child`/`next_sibling` rather than by scanning.
fn walk(
    nodes: &[FlatNode],
    parent: NodeId,
    depth: usize,
    group: Option<&str>,
    out: &mut Vec<Shape>,
) {
    let by_id = |id: NodeId| nodes.iter().find(|n| n.id == id);
    let Some(parent_node) = by_id(parent) else {
        return;
    };
    let mut next = parent_node.first_child;
    while let Some(id) = next {
        let Some(node) = by_id(id) else { break };
        next = node.next_sibling;
        if node.kind != Kind::Container {
            continue;
        }
        let Some(kind) = node.name.as_deref().and_then(ShapeKind::from_tag) else {
            continue;
        };
        let shape = Shape {
            node: node.id,
            kind,
            id: attr_of(node, "data-id"),
            group: group.map(str::to_string),
            depth,
            attrs: node.attrs.clone(),
            text: (kind == ShapeKind::Text)
                .then(|| characters(nodes, id))
                .filter(|t| !t.is_empty()),
        };
        let own_id = shape.id.clone();
        out.push(shape);
        if kind == ShapeKind::Group {
            walk(nodes, id, depth + 1, own_id.as_deref(), out);
        }
    }
}

/// The characters under an element, whitespace runs collapsed to one
/// space and the ends trimmed, on each line a `data-break` starts.
fn characters(nodes: &[FlatNode], parent: NodeId) -> String {
    let mut raw = String::new();
    gather(nodes, parent, &mut raw);
    raw.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn gather(nodes: &[FlatNode], parent: NodeId, out: &mut String) {
    let by_id = |id: NodeId| nodes.iter().find(|n| n.id == id);
    let mut next = by_id(parent).and_then(|n| n.first_child);
    while let Some(id) = next {
        let Some(node) = by_id(id) else { break };
        next = node.next_sibling;
        match node.kind {
            Kind::Str => out.push_str(node.text.as_deref().unwrap_or("")),
            // A `<tspan>` is a run of its own — a wrapped label's line —
            // so a word does not run into the next one's.
            Kind::Container => {
                out.push(if attr_of(node, "data-break").is_some() {
                    '\n'
                } else {
                    ' '
                });
                gather(nodes, id, out);
                out.push(' ');
            }
            _ => {}
        }
    }
}

pub(crate) fn attr_of(node: &FlatNode, name: &str) -> Option<String> {
    node.attrs
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.clone())
}
