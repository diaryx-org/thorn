//! Every fixture under `tests/fixtures/` conforms to the profile, opens with
//! its shapes in paint order, and survives an add and an undo byte for byte.

use svg_editor_core::{Drawing, Rect, ShapeKind};

fn fixtures() -> Vec<(String, String)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let mut files = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "svg"))
        .collect::<Vec<_>>();
    files.sort();
    assert!(!files.is_empty());
    files
        .into_iter()
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect()
}

#[test]
fn every_fixture_conforms() {
    for (name, src) in fixtures() {
        let drawing = Drawing::open(&src).unwrap();
        assert_eq!(drawing.check(), [], "{name}");
    }
}

#[test]
fn an_add_and_an_undo_leave_the_bytes_as_they_were() {
    for (name, src) in fixtures() {
        let mut drawing = Drawing::open(&src).unwrap();
        let before = drawing.shapes().len();
        let id = drawing
            .add_rect(Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            })
            .unwrap();
        assert_eq!(drawing.shapes().len(), before + 1, "{name}");
        assert_eq!(
            drawing.shapes().last().unwrap().id.as_deref(),
            Some(id.as_str())
        );
        assert_eq!(
            drawing.check(),
            [],
            "{name}: the file the editor writes conforms"
        );
        assert!(drawing.undo().unwrap());
        assert_eq!(drawing.source(), src, "{name}");
    }
}

#[test]
fn boxes_and_arrow_reads_in_paint_order() {
    let src = include_str!("fixtures/boxes-and-arrow.svg");
    let drawing = Drawing::open(src).unwrap();
    let kinds = drawing.shapes().iter().map(|s| s.kind).collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            ShapeKind::Rect,
            ShapeKind::Text,
            ShapeKind::Ellipse,
            ShapeKind::Text,
            ShapeKind::Line
        ]
    );
    // The `<path>` inside `<defs><marker>` is not a shape: it is the arrowhead
    // the line references, and the editor preserves it without modelling it.
    assert!(drawing.shapes().iter().all(|s| s.kind != ShapeKind::Path));
    let arrow = drawing.shape("s5").unwrap();
    assert_eq!(arrow.attr("data-from"), Some("s1"));
    assert_eq!(arrow.number("x2"), Some(198.0));
}
