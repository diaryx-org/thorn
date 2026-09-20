//! The editor: a `twig::Editor` over the bytes, a shape list read back after
//! every edit, and the gestures. Each gesture is one twig operation and so
//! one undo step; undo is twig's, and the editor keeps no second history.
//!
//! Every gesture addresses its node by a *locator* computed fresh from the
//! tree — twig's locators are index paths (`"4.3"`) that count every node,
//! whitespace text included, so one is only valid until the next edit.

use std::fmt::Write as _;

use twig::{Editor, FlatNode, Format, Kind, NodeId};

use crate::number;
use crate::profile::{self, Finding};
use crate::shape::{self, Shape};

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
    /// twig refused the edit: the target's span is not editable — a
    /// self-closed `<svg/>` has no interior to insert into — or the edit
    /// would have produced a document that no longer parses and was rolled
    /// back.
    #[error("twig refused the edit: {0:?}")]
    Edit(twig::Error),
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
        let editor = Editor::new_str(source, Format::Xml).map_err(Error::Parse)?;
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
        let id = self.mint_id();
        let mut markup = String::new();
        write!(
            markup,
            r#"<rect x="{}" y="{}" width="{}" height="{}" data-id="{id}"/>"#,
            number::fmt(rect.x),
            number::fmt(rect.y),
            number::fmt(rect.width),
            number::fmt(rect.height),
        )
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

    #[test]
    fn deleting_a_group_takes_its_members() {
        let mut d = Drawing::open("<svg viewBox=\"0 0 9 9\">\n  <g data-id=\"s2\"><circle cx=\"5\" cy=\"5\" r=\"1\" data-id=\"s3\"/></g>\n</svg>\n").unwrap();
        d.delete("s2").unwrap();
        assert!(d.shapes().is_empty());
    }
}
