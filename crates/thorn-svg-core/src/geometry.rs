//! Geometry the gestures compute: a shape's bounds from its attributes, and
//! the attributes a moved or resized shape carries. Pure functions over
//! strings and numbers; the tree is not touched here.

use crate::number;
use crate::shape::{Shape, ShapeKind};

/// An axis-aligned box in user units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The bounds a shape's own attributes state. `None` for a kind whose extent
/// is not in its attributes — a `<path>`'s `d`, a `<g>`'s members — or a
/// shape missing the attributes its kind needs. Ignores `transform`.
pub fn bounds(shape: &Shape) -> Option<Bounds> {
    let n = |name: &str| shape.number(name);
    let or0 = |name: &str| n(name).unwrap_or(0.0);
    Some(match shape.kind {
        ShapeKind::Rect | ShapeKind::Image => Bounds {
            x: or0("x"),
            y: or0("y"),
            width: n("width")?,
            height: n("height")?,
        },
        ShapeKind::Ellipse => {
            let (cx, cy, rx, ry) = (or0("cx"), or0("cy"), n("rx")?, n("ry")?);
            Bounds {
                x: cx - rx,
                y: cy - ry,
                width: 2.0 * rx,
                height: 2.0 * ry,
            }
        }
        ShapeKind::Circle => {
            let (cx, cy, r) = (or0("cx"), or0("cy"), n("r")?);
            Bounds {
                x: cx - r,
                y: cy - r,
                width: 2.0 * r,
                height: 2.0 * r,
            }
        }
        ShapeKind::Line => {
            let (x1, y1, x2, y2) = (or0("x1"), or0("y1"), or0("x2"), or0("y2"));
            Bounds {
                x: x1.min(x2),
                y: y1.min(y2),
                width: (x2 - x1).abs(),
                height: (y2 - y1).abs(),
            }
        }
        ShapeKind::Polyline | ShapeKind::Polygon => {
            let pts = points(shape.attr("points")?)?;
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for (x, y) in &pts {
                x0 = x0.min(*x);
                y0 = y0.min(*y);
                x1 = x1.max(*x);
                y1 = y1.max(*y);
            }
            Bounds {
                x: x0,
                y: y0,
                width: x1 - x0,
                height: y1 - y0,
            }
        }
        // A label's extent is its font's to say; its anchor is all the
        // attributes carry.
        ShapeKind::Text => Bounds {
            x: or0("x"),
            y: or0("y"),
            width: 0.0,
            height: 0.0,
        },
        ShapeKind::Path | ShapeKind::Group => return None,
    })
}

/// The geometry attributes of `shape` moved by `(dx, dy)`, as `(name,
/// value)` pairs to write back. `None` for a kind whose position is not in
/// its attributes — a `<path>` or a `<g>` moves by `transform`, which is not
/// yet a gesture (docs/tasks/hit-testing.md).
pub fn moved(shape: &Shape, dx: f64, dy: f64) -> Option<Vec<(&'static str, String)>> {
    let shift = |name: &'static str, by: f64| -> Option<(&'static str, String)> {
        shape.number(name).map(|v| (name, number::fmt(v + by)))
    };
    Some(match shape.kind {
        ShapeKind::Rect | ShapeKind::Image | ShapeKind::Text => {
            // An absent `x` or `y` is 0 and stays absent when the shift is 0.
            let x = shape.number("x").unwrap_or(0.0) + dx;
            let y = shape.number("y").unwrap_or(0.0) + dy;
            vec![("x", number::fmt(x)), ("y", number::fmt(y))]
        }
        ShapeKind::Ellipse | ShapeKind::Circle => {
            let cx = shape.number("cx").unwrap_or(0.0) + dx;
            let cy = shape.number("cy").unwrap_or(0.0) + dy;
            vec![("cx", number::fmt(cx)), ("cy", number::fmt(cy))]
        }
        ShapeKind::Line => ["x1", "x2"]
            .into_iter()
            .filter_map(|n| shift(n, dx))
            .chain(["y1", "y2"].into_iter().filter_map(|n| shift(n, dy)))
            .collect(),
        ShapeKind::Polyline | ShapeKind::Polygon => {
            let pts = points(shape.attr("points")?)?;
            vec![(
                "points",
                fmt_points(pts.iter().map(|(x, y)| (x + dx, y + dy))),
            )]
        }
        ShapeKind::Path | ShapeKind::Group => return None,
    })
}

/// The geometry attributes of `shape` fitted to `to`, as `(name, value)`
/// pairs to write back. A line keeps its direction (which corner each end
/// is at); a circle takes the smaller side; a label moves its anchor and
/// has no size. `None` as for [`moved`].
pub fn resized(shape: &Shape, to: Bounds) -> Option<Vec<(&'static str, String)>> {
    let f = number::fmt;
    Some(match shape.kind {
        ShapeKind::Rect | ShapeKind::Image => vec![
            ("x", f(to.x)),
            ("y", f(to.y)),
            ("width", f(to.width)),
            ("height", f(to.height)),
        ],
        ShapeKind::Ellipse => vec![
            ("cx", f(to.x + to.width / 2.0)),
            ("cy", f(to.y + to.height / 2.0)),
            ("rx", f(to.width / 2.0)),
            ("ry", f(to.height / 2.0)),
        ],
        ShapeKind::Circle => {
            let r = to.width.min(to.height) / 2.0;
            vec![("cx", f(to.x + r)), ("cy", f(to.y + r)), ("r", f(r))]
        }
        ShapeKind::Line => {
            let flip_x = shape.number("x1").unwrap_or(0.0) > shape.number("x2").unwrap_or(0.0);
            let flip_y = shape.number("y1").unwrap_or(0.0) > shape.number("y2").unwrap_or(0.0);
            let (xa, xb) = if flip_x {
                (to.x + to.width, to.x)
            } else {
                (to.x, to.x + to.width)
            };
            let (ya, yb) = if flip_y {
                (to.y + to.height, to.y)
            } else {
                (to.y, to.y + to.height)
            };
            vec![("x1", f(xa)), ("y1", f(ya)), ("x2", f(xb)), ("y2", f(yb))]
        }
        ShapeKind::Text => vec![("x", f(to.x)), ("y", f(to.y))],
        ShapeKind::Polyline | ShapeKind::Polygon => {
            let pts = points(shape.attr("points")?)?;
            let from = bounds(shape)?;
            let sx = if from.width == 0.0 {
                1.0
            } else {
                to.width / from.width
            };
            let sy = if from.height == 0.0 {
                1.0
            } else {
                to.height / from.height
            };
            vec![(
                "points",
                fmt_points(
                    pts.iter()
                        .map(|(x, y)| (to.x + (x - from.x) * sx, to.y + (y - from.y) * sy)),
                ),
            )]
        }
        ShapeKind::Path | ShapeKind::Group => return None,
    })
}

/// A `points` list: pairs separated by whitespace and/or commas.
pub(crate) fn points(text: &str) -> Option<Vec<(f64, f64)>> {
    let nums = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    if nums.len() % 2 != 0 {
        return None;
    }
    Some(nums.chunks(2).map(|p| (p[0], p[1])).collect())
}

/// `points` as the profile writes it: `x,y` pairs separated by one space.
fn fmt_points(pts: impl Iterator<Item = (f64, f64)>) -> String {
    pts.map(|(x, y)| format!("{},{}", number::fmt(x), number::fmt(y)))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use twig::NodeId;

    fn shape(kind: ShapeKind, attrs: &[(&str, &str)]) -> Shape {
        Shape {
            node: NodeId(0),
            kind,
            id: Some("s".into()),
            group: None,
            depth: 0,
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), Some(v.to_string())))
                .collect(),
        }
    }

    #[test]
    fn bounds_of_each_kind() {
        let b = |k, a: &[(&str, &str)]| bounds(&shape(k, a)).unwrap();
        assert_eq!(
            b(
                ShapeKind::Rect,
                &[("x", "1"), ("y", "2"), ("width", "3"), ("height", "4")]
            ),
            Bounds {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0
            }
        );
        assert_eq!(
            b(
                ShapeKind::Ellipse,
                &[("cx", "5"), ("cy", "5"), ("rx", "2"), ("ry", "1")]
            ),
            Bounds {
                x: 3.0,
                y: 4.0,
                width: 4.0,
                height: 2.0
            }
        );
        assert_eq!(
            b(ShapeKind::Circle, &[("cx", "5"), ("cy", "5"), ("r", "2")]),
            Bounds {
                x: 3.0,
                y: 3.0,
                width: 4.0,
                height: 4.0
            }
        );
        assert_eq!(
            b(
                ShapeKind::Line,
                &[("x1", "9"), ("y1", "1"), ("x2", "1"), ("y2", "3")]
            ),
            Bounds {
                x: 1.0,
                y: 1.0,
                width: 8.0,
                height: 2.0
            }
        );
        assert_eq!(
            b(ShapeKind::Polygon, &[("points", "0,0 4,0 4,3")]),
            Bounds {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 3.0
            }
        );
        assert!(
            bounds(&shape(ShapeKind::Rect, &[("x", "1")])).is_none(),
            "no width"
        );
        assert!(bounds(&shape(ShapeKind::Path, &[("d", "M0 0")])).is_none());
    }

    #[test]
    fn moved_shifts_only_the_position() {
        let m = |k, a: &[(&str, &str)]| moved(&shape(k, a), 1.5, -2.0).unwrap();
        assert_eq!(
            m(ShapeKind::Rect, &[("x", "1"), ("width", "3")]),
            [("x", "2.5".to_string()), ("y", "-2".to_string())]
        );
        assert_eq!(
            m(ShapeKind::Circle, &[("cx", "5"), ("cy", "5"), ("r", "2")]),
            [("cx", "6.5".to_string()), ("cy", "3".to_string())]
        );
        assert_eq!(
            m(
                ShapeKind::Line,
                &[("x1", "0"), ("y1", "0"), ("x2", "4"), ("y2", "4")]
            ),
            [
                ("x1", "1.5".to_string()),
                ("x2", "5.5".to_string()),
                ("y1", "-2".to_string()),
                ("y2", "2".to_string())
            ]
        );
        assert_eq!(
            m(ShapeKind::Polyline, &[("points", "0 0, 4,0")]),
            [("points", "1.5,-2 5.5,-2".to_string())]
        );
        assert!(moved(&shape(ShapeKind::Group, &[]), 1.0, 1.0).is_none());
    }

    #[test]
    fn resized_fits_the_box_and_a_line_keeps_its_direction() {
        let to = Bounds {
            x: 10.0,
            y: 20.0,
            width: 8.0,
            height: 4.0,
        };
        let r = |k, a: &[(&str, &str)]| resized(&shape(k, a), to).unwrap();
        assert_eq!(
            r(ShapeKind::Ellipse, &[("rx", "1"), ("ry", "1")]),
            [
                ("cx", "14".to_string()),
                ("cy", "22".to_string()),
                ("rx", "4".to_string()),
                ("ry", "2".to_string())
            ]
        );
        assert_eq!(
            r(ShapeKind::Circle, &[("r", "1")]),
            [
                ("cx", "12".to_string()),
                ("cy", "22".to_string()),
                ("r", "2".to_string())
            ]
        );
        assert_eq!(
            r(
                ShapeKind::Line,
                &[("x1", "9"), ("y1", "0"), ("x2", "0"), ("y2", "9")]
            ),
            [
                ("x1", "18".to_string()),
                ("y1", "20".to_string()),
                ("x2", "10".to_string()),
                ("y2", "24".to_string())
            ]
        );
        assert_eq!(
            r(ShapeKind::Polygon, &[("points", "0,0 4,0 4,3")]),
            [("points", "10,20 18,20 18,24".to_string())]
        );
    }
}
