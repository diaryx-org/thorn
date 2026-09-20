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

impl Shape {
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
