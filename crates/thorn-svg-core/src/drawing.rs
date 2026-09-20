//! The editor: a `twig::Editor` over the bytes, a shape list read back after
//! every edit, and the gestures. Each gesture is one twig operation and so
//! one undo step; undo is twig's, and the editor keeps no second history.
//!
//! Every gesture addresses its node by a *locator* computed fresh from the
//! tree — twig's locators are index paths (`"4.3"`) that count every node,
//! whitespace text included, so one is only valid until the next edit.

use std::fmt::Write as _;

use twig::{Editor, FlatNode, Format, Kind, NodeId};

use crate::geometry::{self, Bounds};
use crate::number;
use crate::profile::{self, Finding};
use crate::shape::{self, Shape, ShapeKind};

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
    /// The gesture is not defined for this kind of shape: a `<path>` or a
    /// `<g>` has no position in its attributes to move or resize by.
    #[error("{gesture} is not defined for a <{}>", kind.tag())]
    Unsupported {
        gesture: &'static str,
        kind: ShapeKind,
    },
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

/// A drawing being edited.
pub struct Drawing {
    editor: Editor,
    /// The flat tree as of the last edit; refreshed by [`Drawing::reload`].
    nodes: Vec<FlatNode>,
    root: NodeId,
    shapes: Vec<Shape>,
    source: String,
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
            nodes: Vec::new(),
            root: NodeId(0),
            shapes: Vec::new(),
            source: String::new(),
        };
        drawing.reload()?;
        Ok(drawing)
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

    /// Add a label anchored at `(x, y)` as the topmost shape. `text` is
    /// written escaped; it is plain text, not markup. Returns the id.
    pub fn add_text(&mut self, x: f64, y: f64, text: &str) -> Result<String, Error> {
        let f = number::fmt;
        self.add_shape("text", &[("x", f(x)), ("y", f(y))], Some(text))
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
        match content {
            Some(text) => write!(markup, ">{}</{tag}>", escape(text)),
            None => write!(markup, "/>"),
        }
        .expect("writing to a String");
        self.append_to_root(&markup)?;
        Ok(id)
    }

    /// Delete the shape with this `data-id`. One splice, one undo step. The
    /// line's indentation is left where it was (docs/tasks/delete-leaves-its-line.md).
    pub fn delete(&mut self, id: &str) -> Result<(), Error> {
        let node = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?
            .node;
        let locator = self.locator(node);
        self.editor.delete(&locator).map_err(Error::Edit)?;
        self.reload()
    }

    /// The bounds a shape's attributes state, or `None` for a kind whose
    /// extent is not in its attributes (a `<path>`, a `<g>`).
    pub fn bounds(&self, id: &str) -> Option<Bounds> {
        self.shape(id).and_then(geometry::bounds)
    }

    /// The topmost shape within `tolerance` user units of `(x, y)`, in paint
    /// order — the last one painted wins. A member of a group is returned
    /// itself; `Shape::group` names the group for a host that selects
    /// groups whole.
    pub fn hit(&self, x: f64, y: f64, tolerance: f64) -> Option<&Shape> {
        self.shapes
            .iter()
            .rev()
            .find(|s| crate::hit::hits(s, x, y, tolerance))
    }

    /// Move a shape by `(dx, dy)`: one `set_node_attrs`, one undo step,
    /// every other attribute left in place and in order.
    pub fn move_by(&mut self, id: &str, dx: f64, dy: f64) -> Result<(), Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        let updates = geometry::moved(shape, dx, dy).ok_or(Error::Unsupported {
            gesture: "move",
            kind: shape.kind,
        })?;
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)
    }

    /// Fit a shape to `to`: one `set_node_attrs`, one undo step. A line
    /// keeps its direction, a circle takes the smaller side, a label moves
    /// its anchor.
    pub fn resize(&mut self, id: &str, to: Bounds) -> Result<(), Error> {
        let shape = self
            .shape(id)
            .ok_or_else(|| Error::NoSuchShape(id.to_string()))?;
        let updates = geometry::resized(shape, to).ok_or(Error::Unsupported {
            gesture: "resize",
            kind: shape.kind,
        })?;
        self.write_attrs(shape.node, &shape.attrs.clone(), &updates)
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

    /// Write `updates` over `attrs` — replacing a value in place, appending
    /// a name not yet present — through one `set_node_attrs`.
    fn write_attrs(
        &mut self,
        node: NodeId,
        attrs: &[(String, Option<String>)],
        updates: &[(&str, String)],
    ) -> Result<(), Error> {
        let mut merged: Vec<(String, Option<String>)> = attrs.to_vec();
        for (name, value) in updates {
            match merged.iter_mut().find(|(k, _)| k == name) {
                Some(slot) => slot.1 = Some(value.clone()),
                None => merged.push((name.to_string(), Some(value.clone()))),
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
        let taken = self
            .shapes
            .iter()
            .filter_map(|s| s.id.as_deref())
            .filter_map(|id| id.strip_prefix('s'))
            .filter_map(|n| n.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("s{}", taken + 1)
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
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <rect x=\"10\" y=\"10.5\" width=\"80\" height=\"40.125\" data-id=\"s1\"/>\n  <rect x=\"0\" y=\"0\" width=\"1\" height=\"1\" data-id=\"s2\"/>\n</svg>\n"
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
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n  <ellipse cx=\"5\" cy=\"2\" rx=\"5\" ry=\"2\" data-id=\"s1\"/>\n  <line x1=\"1\" y1=\"2\" x2=\"3\" y2=\"4\" data-id=\"s2\"/>\n  <text x=\"5\" y=\"6\" data-id=\"s3\">a &lt; b &amp; &quot;c&quot;</text>\n</svg>\n"
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
            "<svg viewBox=\"0 0 1 1\">\n  <rect x=\"1\" y=\"2\" width=\"3\" height=\"4\" data-id=\"s1\"/>\n</svg>"
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
            "<svg viewBox=\"0 0 9 9\" data-diaryx-drawing=\"1\">\n  <!-- keep me -->\n  \n  <g data-id=\"s2\"><circle cx=\"5\" cy=\"5\" r=\"1\" data-id=\"s3\"/></g>\n</svg>\n"
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
    fn move_rewrites_the_position_and_nothing_else() {
        let mut d = Drawing::open(SCENE).unwrap();
        d.move_by("s1", 5.0, -2.5).unwrap();
        assert_eq!(
            d.source(),
            SCENE.replace(
                "<rect x=\"10\" y=\"10\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>",
                "<rect x=\"15\" y=\"7.5\" width=\"20\" height=\"10\" fill=\"red\" data-id=\"s1\"/>"
            )
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
    fn a_path_has_no_position_to_move() {
        let mut d =
            Drawing::open("<svg viewBox=\"0 0 1 1\"><path d=\"M0 0\" data-id=\"p\"/></svg>")
                .unwrap();
        assert!(matches!(
            d.move_by("p", 1.0, 1.0),
            Err(Error::Unsupported {
                gesture: "move",
                kind: ShapeKind::Path
            })
        ));
        assert!(matches!(
            d.move_by("zz", 1.0, 1.0),
            Err(Error::NoSuchShape(_))
        ));
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
