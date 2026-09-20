//! What is under the pointer: hit-testing over the shape list, and the
//! handle set a selection is resized by. Pure geometry — a canvas on any
//! platform asks these the same question and draws the same answer.

use crate::geometry::{self, Bounds};
use crate::path::{self, Subpath};
use crate::shape::{Shape, ShapeKind};

/// One of the eight resize handles on a selection's bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    /// Every handle, clockwise from the top-left.
    pub const ALL: [Handle; 8] = [
        Handle::TopLeft,
        Handle::Top,
        Handle::TopRight,
        Handle::Right,
        Handle::BottomRight,
        Handle::Bottom,
        Handle::BottomLeft,
        Handle::Left,
    ];

    /// Where this handle sits on `bounds`.
    pub fn position(self, bounds: Bounds) -> (f64, f64) {
        let (x0, y0) = (bounds.x, bounds.y);
        let (x1, y1) = (bounds.x + bounds.width, bounds.y + bounds.height);
        let (xm, ym) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        match self {
            Handle::TopLeft => (x0, y0),
            Handle::Top => (xm, y0),
            Handle::TopRight => (x1, y0),
            Handle::Right => (x1, ym),
            Handle::BottomRight => (x1, y1),
            Handle::Bottom => (xm, y1),
            Handle::BottomLeft => (x0, y1),
            Handle::Left => (x0, ym),
        }
    }

    /// The handle of `bounds` within `tolerance` of `(x, y)`, if any.
    pub fn at(bounds: Bounds, x: f64, y: f64, tolerance: f64) -> Option<Handle> {
        Handle::ALL.into_iter().find(|h| {
            let (hx, hy) = h.position(bounds);
            (hx - x).abs() <= tolerance && (hy - y).abs() <= tolerance
        })
    }

    /// `bounds` after this handle is dragged by `(dx, dy)`, the opposite
    /// edge or corner held still. A drag past the opposite edge flips the
    /// box rather than making a negative size.
    pub fn drag(self, bounds: Bounds, dx: f64, dy: f64) -> Bounds {
        let (mut x0, mut y0) = (bounds.x, bounds.y);
        let (mut x1, mut y1) = (bounds.x + bounds.width, bounds.y + bounds.height);
        match self {
            Handle::TopLeft | Handle::Left | Handle::BottomLeft => x0 += dx,
            Handle::TopRight | Handle::Right | Handle::BottomRight => x1 += dx,
            Handle::Top | Handle::Bottom => {}
        }
        match self {
            Handle::TopLeft | Handle::Top | Handle::TopRight => y0 += dy,
            Handle::BottomLeft | Handle::Bottom | Handle::BottomRight => y1 += dy,
            Handle::Left | Handle::Right => {}
        }
        Bounds {
            x: x0.min(x1),
            y: y0.min(y1),
            width: (x1 - x0).abs(),
            height: (y1 - y0).abs(),
        }
    }
}

/// Whether `(x, y)` is on `shape`, within `tolerance` of its silhouette, in
/// the shape's own coordinates — [`Drawing::hit`](crate::Drawing::hit) maps
/// the point through the shape's transform first. A stroke counts as its
/// centreline; a fill counts as its interior, a closed subpath's by
/// even-odd. `false` for a `<g>`, which is hit through its members.
pub fn hits(shape: &Shape, x: f64, y: f64, tolerance: f64) -> bool {
    let n = |name: &str| shape.number(name).unwrap_or(0.0);
    match shape.kind {
        ShapeKind::Rect | ShapeKind::Image => {
            geometry::bounds(shape).is_some_and(|b| b.expanded(tolerance).contains(x, y))
        }
        ShapeKind::Ellipse | ShapeKind::Circle => {
            let (rx, ry) = if shape.kind == ShapeKind::Circle {
                (n("r"), n("r"))
            } else {
                (n("rx"), n("ry"))
            };
            let (rx, ry) = (rx + tolerance, ry + tolerance);
            if rx <= 0.0 || ry <= 0.0 {
                return false;
            }
            let (u, v) = ((x - n("cx")) / rx, (y - n("cy")) / ry);
            u * u + v * v <= 1.0
        }
        ShapeKind::Line => {
            segment_distance((n("x1"), n("y1")), (n("x2"), n("y2")), (x, y)) <= tolerance
        }
        ShapeKind::Polyline | ShapeKind::Polygon => {
            let Some(pts) = geometry::points(shape.attr("points").unwrap_or("")) else {
                return false;
            };
            hits_subpath(
                &Subpath {
                    points: pts,
                    closed: shape.kind == ShapeKind::Polygon,
                },
                x,
                y,
                tolerance,
            )
        }
        ShapeKind::Path => path::flatten(shape.attr("d").unwrap_or(""))
            .is_some_and(|subs| subs.iter().any(|s| hits_subpath(s, x, y, tolerance))),
        // A label's extent is its font's to say; until the host says, a box
        // around the anchor is what a click can land on.
        ShapeKind::Text => {
            let b = Bounds {
                x: n("x") - TEXT_REACH,
                y: n("y") - TEXT_REACH,
                width: 2.0 * TEXT_REACH,
                height: TEXT_REACH,
            };
            b.expanded(tolerance).contains(x, y)
        }
        ShapeKind::Group => false,
    }
}

/// On the polyline's edges within `tolerance`, or inside it when closed.
fn hits_subpath(sub: &Subpath, x: f64, y: f64, tolerance: f64) -> bool {
    let pts = &sub.points;
    if sub.closed && point_in_polygon(pts, (x, y)) {
        return true;
    }
    let edges = pts.windows(2).map(|w| (w[0], w[1]));
    let closing = sub
        .closed
        .then(|| pts.first().zip(pts.last()).map(|(a, b)| (*b, *a)))
        .flatten();
    edges
        .chain(closing)
        .any(|(a, b)| segment_distance(a, b, (x, y)) <= tolerance)
}

/// How far around a `<text>` anchor a click counts, in user units, until
/// the host supplies measured bounds.
const TEXT_REACH: f64 = 12.0;

impl Bounds {
    /// Whether the box contains the point, edges included.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    /// The box grown by `by` on every side.
    pub fn expanded(&self, by: f64) -> Bounds {
        Bounds {
            x: self.x - by,
            y: self.y - by,
            width: self.width + 2.0 * by,
            height: self.height + 2.0 * by,
        }
    }

    /// The box moved by `(dx, dy)`.
    pub fn offset(&self, dx: f64, dy: f64) -> Bounds {
        Bounds {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let len2 = vx * vx + vy * vy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.0 + t * vx, a.1 + t * vy);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// Even-odd point-in-polygon.
fn point_in_polygon(pts: &[(f64, f64)], p: (f64, f64)) -> bool {
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    for i in 0..pts.len() {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > p.1) != (yj > p.1) && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use twig::NodeId;

    fn shape(kind: ShapeKind, attrs: &[(&str, &str)]) -> Shape {
        Shape {
            node: NodeId(0),
            kind,
            id: None,
            group: None,
            depth: 0,
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), Some(v.to_string())))
                .collect(),
        }
    }

    #[test]
    fn each_kind_hits_its_silhouette() {
        let rect = shape(
            ShapeKind::Rect,
            &[("x", "10"), ("y", "10"), ("width", "20"), ("height", "10")],
        );
        assert!(hits(&rect, 15.0, 15.0, 0.0));
        assert!(hits(&rect, 31.0, 15.0, 2.0), "within tolerance");
        assert!(!hits(&rect, 33.0, 15.0, 2.0));

        let circle = shape(
            ShapeKind::Circle,
            &[("cx", "50"), ("cy", "50"), ("r", "10")],
        );
        assert!(hits(&circle, 57.0, 57.0, 0.0));
        assert!(
            !hits(&circle, 58.0, 58.0, 0.0),
            "the corner of the bounds is outside"
        );
        assert!(hits(&circle, 61.0, 50.0, 2.0));

        let line = shape(
            ShapeKind::Line,
            &[("x1", "0"), ("y1", "0"), ("x2", "10"), ("y2", "10")],
        );
        assert!(hits(&line, 5.0, 5.5, 1.0));
        assert!(
            !hits(&line, 5.0, 8.0, 1.0),
            "inside the bounds but off the line"
        );
        assert!(!hits(&line, 12.0, 12.0, 1.0), "past the end");

        let tri = shape(ShapeKind::Polygon, &[("points", "0,0 10,0 0,10")]);
        assert!(hits(&tri, 2.0, 2.0, 0.0));
        assert!(!hits(&tri, 8.0, 8.0, 0.0));
        assert!(hits(&tri, 5.5, 5.5, 1.0), "on the hypotenuse");
        let open = shape(ShapeKind::Polyline, &[("points", "0,0 10,0 0,10")]);
        assert!(!hits(&open, 2.0, 2.0, 0.0), "a polyline has no interior");

        let text = shape(ShapeKind::Text, &[("x", "100"), ("y", "100")]);
        assert!(hits(&text, 105.0, 95.0, 0.0));
        assert!(!hits(&text, 100.0, 120.0, 0.0));

        let stroke = shape(ShapeKind::Path, &[("d", "M0 0h10")]);
        assert!(hits(&stroke, 5.0, 0.5, 1.0));
        assert!(!hits(&stroke, 5.0, 3.0, 1.0));
        let filled = shape(ShapeKind::Path, &[("d", "M0 0h10v10h-10z")]);
        assert!(
            hits(&filled, 5.0, 5.0, 0.0),
            "a closed subpath has an interior"
        );
        assert!(!hits(
            &shape(ShapeKind::Path, &[("d", "M0 0")]),
            0.0,
            0.0,
            5.0
        ));
        assert!(!hits(&shape(ShapeKind::Group, &[]), 0.0, 0.0, 5.0));
    }

    #[test]
    fn handles_sit_on_the_bounds_and_drag_the_right_edge() {
        let b = Bounds {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };
        assert_eq!(Handle::BottomRight.position(b), (40.0, 60.0));
        assert_eq!(Handle::Left.position(b), (10.0, 40.0));
        assert_eq!(Handle::at(b, 41.0, 59.0, 2.0), Some(Handle::BottomRight));
        assert_eq!(
            Handle::at(b, 25.0, 40.0, 2.0),
            None,
            "the middle is not a handle"
        );
        assert_eq!(
            Handle::Right.drag(b, 5.0, 99.0),
            Bounds {
                x: 10.0,
                y: 20.0,
                width: 35.0,
                height: 40.0
            }
        );
        assert_eq!(
            Handle::TopLeft.drag(b, 5.0, 5.0),
            Bounds {
                x: 15.0,
                y: 25.0,
                width: 25.0,
                height: 35.0
            }
        );
        assert_eq!(
            Handle::Left.drag(b, 40.0, 0.0),
            Bounds {
                x: 40.0,
                y: 20.0,
                width: 10.0,
                height: 40.0
            },
            "flips past the far edge"
        );
    }
}
