//! Every fixture under `tests/fixtures/` conforms to the profile, opens with
//! its shapes in paint order, and survives an add and an undo byte for byte.

use thorn_svg_core::{Bounds, Drawing, Heads, Nib, Order, Rect, Rule, ShapeKind};

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

    // The arrow is bound at both ends: moving the ellipse up brings its
    // head along the rim, and the tail off the box's right edge to face
    // it — one step, back to the same bytes.
    let mut drawing = drawing;
    drawing.move_by("s3", 0.0, -30.0).unwrap();
    let arrow = drawing.shape("s5").unwrap();
    assert_eq!(arrow.attr("x1"), Some("120"));
    assert!(arrow.number("y1").unwrap() < 70.0, "{:?}", arrow.attrs);
    let (x2, y2) = (arrow.number("x2").unwrap(), arrow.number("y2").unwrap());
    let on_rim = ((x2 - 250.0) / 50.0).powi(2) + ((y2 - 40.0) / 30.0).powi(2);
    assert!((on_rim - 1.0).abs() < 0.01, "{x2} {y2}");
    assert_eq!(drawing.check(), []);
    assert!(drawing.undo().unwrap());
    assert_eq!(drawing.source(), src);
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

#[test]
fn an_arrow_is_what_data_arrow_says_or_what_the_file_spells() {
    let src = include_str!("fixtures/arrow-by-data.svg");
    let mut drawing = Drawing::open(src).unwrap();
    assert_eq!(drawing.shape("s3").unwrap().heads(), Some(Heads::End));
    assert_eq!(drawing.shape("s4").unwrap().heads(), Some(Heads::Both));
    assert_eq!(drawing.shape("s1").unwrap().heads(), None);
    // A file that spells `marker-end` itself is an arrow all the same.
    let spelled = Drawing::open(include_str!("fixtures/boxes-and-arrow.svg")).unwrap();
    assert_eq!(spelled.shape("s5").unwrap().heads(), Some(Heads::End));

    // The tool's arrow: one line with data-arrow, one step; its ends bind
    // like any line's.
    let id = drawing
        .add_arrow(10.0, 10.0, 60.5, 20.0, Heads::End)
        .unwrap();
    assert!(drawing.source().contains(&format!(
        "<line x1=\"10\" y1=\"10\" x2=\"60.5\" y2=\"20\" data-arrow=\"end\" data-id=\"{id}\"/>"
    )));
    assert_eq!(drawing.shape(&id).unwrap().heads(), Some(Heads::End));
    assert_eq!(drawing.check(), []);
    assert!(drawing.undo().unwrap());
    assert_eq!(drawing.source(), src);

    // A value the profile does not admit is a finding.
    let odd = src.replace("data-arrow=\"both\"", "data-arrow=\"tail\"");
    let findings = Drawing::open(&odd).unwrap().check();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, Rule::ArrowHeads);
    assert_eq!(findings[0].shape.as_deref(), Some("s4"));
}

#[test]
fn an_ink_stroke_carries_its_centreline_and_keeps_it_under_move_and_resize() {
    let src = include_str!("fixtures/ink.svg");
    let mut drawing = Drawing::open(src).unwrap();
    let stroke = drawing.shape("s1").unwrap().clone();
    assert_eq!(stroke.kind, ShapeKind::Path);
    assert_eq!(stroke.attr("data-ink"), Some("monoline"));
    // The outline is what the nib makes of the centreline: the fixture is
    // the writer's own output, so re-adding the same stroke is the same d.
    let id = drawing
        .add_ink(
            &[
                (20.0, 60.0),
                (50.0, 30.0),
                (90.0, 55.0),
                (140.0, 35.0),
                (170.0, 60.0),
            ],
            &[4.0],
            Nib::Monoline,
        )
        .unwrap();
    assert_eq!(drawing.shape(&id).unwrap().attr("d"), stroke.attr("d"));
    assert!(drawing.undo().unwrap());
    assert_eq!(drawing.source(), src);

    // A move carries the centreline with the outline.
    drawing.move_by("s1", 10.0, -5.0).unwrap();
    let moved = drawing.shape("s1").unwrap();
    assert_eq!(
        moved.attr("data-centreline"),
        Some("M30 55 L60 25 L100 50 L150 30 L180 55")
    );
    assert_eq!(moved.attr("data-widths"), Some("4"));
    assert!(moved.attr("d").unwrap().starts_with("M31.414 56.414 "));
    assert_eq!(drawing.check(), []);
    assert!(drawing.undo().unwrap());
    assert_eq!(drawing.source(), src);

    // A resize scales the centreline and the width, and draws the outline
    // again from them — so a doubled stroke is twice as wide, not a
    // stretched outline.
    let from = drawing.bounds("s1").unwrap();
    drawing
        .resize(
            "s1",
            Bounds {
                x: from.x,
                y: from.y,
                width: from.width * 2.0,
                height: from.height * 2.0,
            },
        )
        .unwrap();
    let resized = drawing.shape("s1").unwrap();
    assert_eq!(resized.attr("data-widths"), Some("8"));
    let centre = resized.attr("data-centreline").unwrap();
    assert!(centre.starts_with("M"), "{centre}");
    let after = drawing.bounds("s1").unwrap();
    assert!((after.width - from.width * 2.0).abs() < 0.2, "{after:?}");
    assert!((after.height - from.height * 2.0).abs() < 0.2, "{after:?}");
    assert_eq!(drawing.check(), []);
    assert!(drawing.undo().unwrap());
    assert_eq!(drawing.source(), src);

    // The vocabulary is closed, and a stroke without its centreline is a
    // finding: the outline alone cannot be drawn again.
    let odd = src.replace("data-ink=\"monoline\"", "data-ink=\"pen\"");
    let findings = Drawing::open(&odd).unwrap().check();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].rule, Rule::InkNib);
    let bare = src.replace(" data-widths=\"4\"", "");
    let findings = Drawing::open(&bare).unwrap().check();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].rule, Rule::InkNib);

    // No points, no stroke.
    assert!(drawing.add_ink(&[], &[4.0], Nib::Monoline).is_err());
}
