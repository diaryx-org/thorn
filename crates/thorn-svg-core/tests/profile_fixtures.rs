//! Every fixture under `tests/fixtures/` conforms to the profile, opens with
//! its shapes in paint order, and survives an add and an undo byte for byte.

use thorn_svg_core::{Bounds, Drawing, Order, Rect, ShapeKind};

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

/// Every gesture on every shape of every fixture is one undo step back to
/// the same bytes, and the file it leaves conforms.
#[test]
fn every_gesture_is_one_step_back_to_the_same_bytes() {
    for (name, src) in fixtures() {
        let mut drawing = Drawing::open(&src).unwrap();
        let ids = drawing
            .shapes()
            .iter()
            .filter_map(|s| s.id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            let kind = drawing.shape(&id).unwrap().kind;
            let mut steps = 0;
            if drawing.move_by(&id, 3.0, -1.5).is_ok() {
                steps += 1;
                assert_eq!(drawing.check(), [], "{name}: after moving {id}");
            }
            let to = Bounds {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 20.0,
            };
            if drawing.resize(&id, to).is_ok() {
                steps += 1;
                assert_eq!(drawing.check(), [], "{name}: after resizing {id}");
            }
            if drawing.reorder(&id, Order::ToFront).unwrap() {
                steps += 1;
            }
            if drawing.reorder(&id, Order::ToBack).unwrap() {
                steps += 1;
            }
            for _ in 0..steps {
                assert!(drawing.undo().unwrap(), "{name}: undoing {kind:?} {id}");
            }
            assert_eq!(drawing.source(), src, "{name}: after {kind:?} {id}");
        }
    }
}
