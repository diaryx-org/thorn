//! A `<text>`'s box, measured. The core has no fonts; a label's extent is
//! its face's to say, and the face is the host's — the same one it draws
//! with, or the box and the glyphs disagree. So a [`Drawing`] takes a
//! [`Measure`] from its host and asks it, once per distinct label as it
//! stands, for the box of the label laid out in its own coordinates;
//! without one it falls back to a nominal box from `font-size` and the
//! character count.
//!
//! [`Usvg`] is the measurer for a host that draws with resvg, behind the
//! `usvg` feature: the same layout, over the system's fonts, so the box is
//! exactly what is on screen.
//!
//! [`Drawing`]: crate::Drawing

use crate::geometry::Bounds;

/// The face a label lays out in, as the host resolved it.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// The size, in the label's user units.
    pub size: f64,
    /// The face's family name — `Helvetica`, not the `sans-serif` that
    /// asked for it.
    pub family: String,
    /// The face's PostScript name — `Helvetica-Bold` — which a platform's
    /// font API instantiates exactly.
    pub post_script_name: String,
}

/// A host's text layout, asked for a label's box.
pub trait Measure: Send + Sync {
    /// The box of the one `<text>` in `svg` — a small document the drawing
    /// writes: the `<svg>` element's attributes, every `<style>` and
    /// `<defs>`, the label's enclosing `<g>`s with their `transform`s
    /// taken off, and the label itself with its `transform` taken off, so
    /// it lays out in its own coordinates. The box is in those. `None`
    /// when the label lays out to nothing — no characters, no face at all.
    fn measure(&self, svg: &str) -> Option<Bounds>;

    /// The face and size the one `<text>` in `svg` lays out in — what a
    /// stylesheet gave it, resolved to a face the host has, which its
    /// attributes cannot say. `None` when the host cannot tell; the
    /// drawing then takes the nominal 12, in no face in particular.
    fn font(&self, svg: &str) -> Option<Font> {
        let _ = svg;
        None
    }
}

/// resvg's layout, over the system's fonts — what a host drawing through
/// resvg sees. The fonts are loaded once per process, on the first label,
/// and shared by every drawing after.
#[cfg(feature = "usvg")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Usvg;

#[cfg(feature = "usvg")]
impl Measure for Usvg {
    fn measure(&self, svg: &str) -> Option<Bounds> {
        let tree = parse(svg)?;
        first_text(tree.root()).map(|t| {
            let r = t.bounding_box();
            Bounds {
                x: r.x() as f64,
                y: r.y() as f64,
                width: r.width() as f64,
                height: r.height() as f64,
            }
        })
    }

    fn font(&self, svg: &str) -> Option<Font> {
        let tree = parse(svg)?;
        let text = first_text(tree.root())?;
        let span = text.layouted().first()?;
        let glyph = span.positioned_glyphs.first()?;
        let face = tree.fontdb().face(glyph.font)?;
        Some(Font {
            size: span.font_size.get() as f64,
            family: face
                .families
                .first()
                .map(|(name, _)| name.clone())
                .unwrap_or_default(),
            post_script_name: face.post_script_name.clone(),
        })
    }
}

#[cfg(feature = "usvg")]
fn parse(svg: &str) -> Option<usvg::Tree> {
    use std::sync::{Arc, OnceLock};
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    let fontdb = FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone();
    let options = usvg::Options {
        fontdb,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(svg, &options).ok()
}

/// The first `<text>`, in the group's own coordinates — the groups the
/// document writes carry no transform, so these are the label's.
#[cfg(feature = "usvg")]
fn first_text(group: &usvg::Group) -> Option<&usvg::Text> {
    group.children().iter().find_map(|node| match node {
        usvg::Node::Text(text) => Some(text.as_ref()),
        usvg::Node::Group(g) => first_text(g),
        _ => None,
    })
}

#[cfg(all(test, feature = "usvg"))]
mod tests {
    use super::*;

    #[test]
    fn usvg_measures_a_label_at_the_origin() {
        let doc = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\">\
                   <text font-size=\"20\">Hello</text></svg>";
        // A machine with no fonts at all lays out nothing; that is the
        // fallback's case, not this test's.
        let Some(b) = Usvg.measure(doc) else {
            eprintln!("no system fonts: nothing to measure");
            return;
        };
        assert!(b.width > 30.0 && b.width < 80.0, "{b:?}");
        assert!(
            b.y < 0.0 && b.y > -20.0,
            "the cap height is above the baseline: {b:?}"
        );
        assert!(b.y + b.height >= -1.0, "{b:?}");
        let font = Usvg.font(doc).unwrap();
        assert_eq!(font.size, 20.0);
        assert!(
            !font.family.is_empty() && !font.post_script_name.is_empty(),
            "{font:?}"
        );
        let styled = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\">\
                      <style>text { font: 16px sans-serif }</style><text>Hello</text></svg>";
        let font = Usvg.font(styled).unwrap();
        assert_eq!(font.size, 16.0, "from the stylesheet");
        assert_ne!(font.family, "sans-serif", "resolved to a face: {font:?}");
        assert!(
            Usvg.measure("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>")
                .is_none()
        );
    }
}
