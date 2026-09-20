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
            Self::Text => &["x", "y"],
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
}

impl Shape {
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
        };
        let own_id = shape.id.clone();
        out.push(shape);
        if kind == ShapeKind::Group {
            walk(nodes, id, depth + 1, own_id.as_deref(), out);
        }
    }
}

pub(crate) fn attr_of(node: &FlatNode, name: &str) -> Option<String> {
    node.attrs
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.clone())
}
