//! Geometry the gestures compute: a shape's bounds from its attributes, and
//! the attributes a moved or resized shape carries. Pure functions over
//! strings and numbers; the tree is not touched here. Everything is in the
//! shape's own coordinates — its `transform` and its groups' are
//! [`Drawing`](crate::Drawing)'s to compose, since only it knows the chain.

use crate::ink;
use crate::number;
use crate::path::{self, Subpath};
use crate::shape::{Shape, ShapeKind};
use crate::transform::Transform;

/// An axis-aligned box in user units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The bounds a shape's own attributes state — a `<path>`'s from its `d`
/// flattened. `None` for a `<g>`, whose extent is its members', or a shape
/// missing the attributes its kind needs. Ignores `transform`.
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
            Bounds::around(pts.iter().copied())?
        }
        ShapeKind::Path => {
            let subs = path::flatten(shape.attr("d")?)?;
            Bounds::around(subs.iter().flat_map(|s| s.points.iter().copied()))?
        }
        ShapeKind::Text => nominal_text(shape),
        ShapeKind::Group => return None,
    })
}

/// The size a `<text>` lays out at when nothing says: usvg's default,
/// which is what the canvas draws with.
const DEFAULT_FONT_SIZE: f64 = 12.0;

/// A label's box without a font: `font-size` (12 when unset) tall, with
/// the baseline four fifths of the way down, and six tenths of that per
/// character wide — the average of a text face — placed by `text-anchor`.
/// What [`Drawing::bounds`](crate::Drawing::bounds) falls back to when no
/// [`Measure`](crate::measure::Measure) is installed; every measured face
/// is within a few units of it.
fn nominal_text(shape: &Shape) -> Bounds {
    let (x, y) = (
        shape.number("x").unwrap_or(0.0),
        shape.number("y").unwrap_or(0.0),
    );
    let size = shape
        .attr("font-size")
        .and_then(|v| v.trim().trim_end_matches("px").parse::<f64>().ok())
        .unwrap_or(DEFAULT_FONT_SIZE);
    // A line per `\n`, and, wrapped to `data-width`, as many more as that
    // takes; lines are 1.2 sizes apart.
    let wrap = shape.number("data-width").filter(|w| *w > 0.0);
    let (mut width, mut lines) = (0.0f64, 0.0f64);
    for paragraph in shape.text.as_deref().unwrap_or("").lines() {
        let needed = 0.6 * size * paragraph.chars().count() as f64;
        match wrap {
            Some(w) if w < needed => {
                lines += (needed / w).ceil();
                width = width.max(w);
            }
            _ => {
                lines += 1.0;
                width = width.max(needed);
            }
        }
    }
    let lines = lines.max(1.0);
    let left = match shape.attr("text-anchor").map(str::trim) {
        Some("middle") => x - width / 2.0,
        Some("end") => x - width,
        _ => x,
    };
    Bounds {
        x: left,
        y: y - 0.8 * size,
        width,
        height: size + (lines - 1.0) * LINE_HEIGHT * size,
    }
}

/// A wrapped label's line pitch, in font sizes — what its `<tspan>`s'
/// `dy` is written as.
pub const LINE_HEIGHT: f64 = 1.2;

impl Bounds {
    /// The smallest box around the points; `None` for none.
    pub fn around(points: impl IntoIterator<Item = (f64, f64)>) -> Option<Bounds> {
        let mut it = points.into_iter();
        let (mut x0, mut y0) = it.next()?;
        let (mut x1, mut y1) = (x0, y0);
        for (x, y) in it {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        Some(Bounds {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }

    /// The smallest box around both.
    pub fn union(&self, other: &Bounds) -> Bounds {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.width).max(other.x + other.width);
        let y1 = (self.y + self.height).max(other.y + other.height);
        Bounds {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }

    /// The box around this box's corners after `t` — its extent in the
    /// space `t` maps into. Exact for a translate or a scale; a rotation
    /// gets the box around the rotated box.
    pub fn transformed(&self, t: &Transform) -> Bounds {
        let (x1, y1) = (self.x + self.width, self.y + self.height);
        Bounds::around([
            t.apply(self.x, self.y),
            t.apply(x1, self.y),
            t.apply(x1, y1),
            t.apply(self.x, y1),
        ])
        .expect("four corners")
    }
}

/// The shape's silhouette as polylines in its own coordinates, for an
/// extent under a transform that is not a box's to keep — a rotated
/// ellipse is not the box around its rotated box. `None` for a `<g>` and
/// for what does not parse; a label is its anchor, since its extent is its
/// font's.
pub fn outline(shape: &Shape) -> Option<Vec<Subpath>> {
    let n = |name: &str| shape.number(name).unwrap_or(0.0);
    let closed = |points| Subpath {
        points,
        closed: true,
    };
    Some(match shape.kind {
        ShapeKind::Rect | ShapeKind::Image | ShapeKind::Text => {
            let b = bounds(shape)?;
            let (x1, y1) = (b.x + b.width, b.y + b.height);
            vec![closed(vec![(b.x, b.y), (x1, b.y), (x1, y1), (b.x, y1)])]
        }
        ShapeKind::Ellipse | ShapeKind::Circle => {
            let b = bounds(shape)?;
            let (rx, ry) = (b.width / 2.0, b.height / 2.0);
            let (cx, cy) = (n("cx"), n("cy"));
            // Enough chords that a selection box around a rotated ellipse
            // is within a hair of it.
            let pts = (0..48)
                .map(|i| {
                    let t = i as f64 / 48.0 * std::f64::consts::TAU;
                    (cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect();
            vec![closed(pts)]
        }
        ShapeKind::Line => vec![Subpath {
            points: vec![(n("x1"), n("y1")), (n("x2"), n("y2"))],
            closed: false,
        }],
        ShapeKind::Polyline | ShapeKind::Polygon => vec![Subpath {
            points: points(shape.attr("points")?)?,
            closed: shape.kind == ShapeKind::Polygon,
        }],
        ShapeKind::Path => path::flatten(shape.attr("d")?)?,
        ShapeKind::Group => return None,
    })
}

/// The shape's own `transform`, the identity when absent or unparseable.
pub fn own_transform(shape: &Shape) -> Transform {
    shape
        .attr("transform")
        .and_then(Transform::parse)
        .unwrap_or_default()
}

/// One attribute to write back: a value, or `None` to take the attribute
/// off — how an identity `transform` is written.
pub type Update = (&'static str, Option<String>);

/// The attributes of `shape` moved by `(dx, dy)`. A kind with its position
/// in its attributes shifts them, and the delta is in the shape's own
/// coordinates; a `<path>` or a `<g>` has none, so it gets a `translate`
/// composed onto its `transform`, and the delta is in its parent's. `None`
/// for a points list that does not parse.
pub fn moved(shape: &Shape, dx: f64, dy: f64) -> Option<Vec<Update>> {
    let shift = |name: &'static str, by: f64| -> Option<Update> {
        shape
            .number(name)
            .map(|v| (name, Some(number::fmt(v + by))))
    };
    Some(match shape.kind {
        ShapeKind::Rect | ShapeKind::Image | ShapeKind::Text => {
            // An absent `x` or `y` is 0 and stays absent when the shift is 0.
            let x = shape.number("x").unwrap_or(0.0) + dx;
            let y = shape.number("y").unwrap_or(0.0) + dy;
            vec![("x", Some(number::fmt(x))), ("y", Some(number::fmt(y)))]
        }
        ShapeKind::Ellipse | ShapeKind::Circle => {
            let cx = shape.number("cx").unwrap_or(0.0) + dx;
            let cy = shape.number("cy").unwrap_or(0.0) + dy;
            vec![("cx", Some(number::fmt(cx))), ("cy", Some(number::fmt(cy)))]
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
                Some(fmt_points(pts.iter().map(|(x, y)| (x + dx, y + dy)))),
            )]
        }
        ShapeKind::Path | ShapeKind::Group => {
            let t = own_transform(shape).then(&Transform::translate(dx, dy));
            baked(shape, &t).unwrap_or_else(|| vec![("transform", t.fmt())])
        }
    })
}

/// The attributes of `shape` with `t` — its own `transform` included, and
/// replaced — applied to them, so the element draws the same with the
/// attribute gone: a box's corners and sides, an ellipse's centre and
/// radii, a `d` rewritten (see [`path::transformed`]). Only an
/// axis-aligned `t` bakes; a rotation or a skew has no attributes to go
/// into, and `None` says keep the `transform`. So does a circle under an
/// unequal scale (it would be an ellipse), a label under a flip or an
/// unequal scale, and one under a scale with no `font-size` of its own to
/// scale. `stroke-width` is left alone, as a resize leaves it, and so is a
/// box's corner radius. A `<g>` has nothing to bake into.
pub fn baked(shape: &Shape, t: &Transform) -> Option<Vec<Update>> {
    if !t.is_axis_aligned() {
        return None;
    }
    let f = |v: f64| Some(number::fmt(v));
    let n = |name: &str| shape.number(name).unwrap_or(0.0);
    let (sx, sy) = (t.a.abs(), t.d.abs());
    let mut updates: Vec<Update> = match shape.kind {
        ShapeKind::Rect | ShapeKind::Image => {
            let (x0, y0) = t.apply(n("x"), n("y"));
            let (x1, y1) = t.apply(n("x") + n("width"), n("y") + n("height"));
            // A corner's radius is kept, as a stroke's width is: a box
            // resized is the same box with round corners, not a scaled one.
            vec![
                ("x", f(x0.min(x1))),
                ("y", f(y0.min(y1))),
                ("width", f((x1 - x0).abs())),
                ("height", f((y1 - y0).abs())),
            ]
        }
        ShapeKind::Ellipse => {
            let (cx, cy) = t.apply(n("cx"), n("cy"));
            vec![
                ("cx", f(cx)),
                ("cy", f(cy)),
                ("rx", f(n("rx") * sx)),
                ("ry", f(n("ry") * sy)),
            ]
        }
        ShapeKind::Circle => {
            if (sx - sy).abs() > 1e-9 {
                return None;
            }
            let (cx, cy) = t.apply(n("cx"), n("cy"));
            vec![("cx", f(cx)), ("cy", f(cy)), ("r", f(n("r") * sx))]
        }
        ShapeKind::Line => {
            let (x1, y1) = t.apply(n("x1"), n("y1"));
            let (x2, y2) = t.apply(n("x2"), n("y2"));
            vec![("x1", f(x1)), ("y1", f(y1)), ("x2", f(x2)), ("y2", f(y2))]
        }
        ShapeKind::Polyline | ShapeKind::Polygon => {
            let pts = points(shape.attr("points")?)?;
            vec![(
                "points",
                Some(fmt_points(pts.iter().map(|&(x, y)| t.apply(x, y)))),
            )]
        }
        ShapeKind::Path => match shape.attr("data-ink").and_then(ink::Nib::from_value) {
            // An ink stroke's `d` is the nib's, not a hand's: the centreline
            // moves with the transform, the widths by its scale, and the
            // outline is drawn again from them, so the three never drift.
            Some(nib) => {
                let centreline = path::transformed(shape.attr("data-centreline")?, t)?;
                let scale = (sx * sy).sqrt();
                let widths: Vec<f64> = ink::parse_widths(shape.attr("data-widths")?)?
                    .into_iter()
                    .map(|w| w * scale)
                    .collect();
                let points = ink::centreline_points(&centreline)?;
                vec![
                    ("d", Some(ink::outline(&points, &widths, nib)?)),
                    ("data-centreline", Some(centreline)),
                    ("data-widths", Some(ink::widths(&widths))),
                ]
            }
            None => vec![("d", Some(path::transformed(shape.attr("d")?, t)?))],
        },
        ShapeKind::Text => {
            if t.a <= 0.0 || (t.a - t.d).abs() > 1e-9 {
                return None;
            }
            let (x, y) = t.apply(n("x"), n("y"));
            let mut u = vec![("x", f(x)), ("y", f(y))];
            if (t.a - 1.0).abs() > 1e-9 {
                let size = shape
                    .attr("font-size")
                    .and_then(|v| v.trim().trim_end_matches("px").parse::<f64>().ok())?;
                u.push(("font-size", f(size * t.a)));
            }
            u
        }
        ShapeKind::Group => return None,
    };
    updates.push(("transform", None));
    Some(updates)
}

/// The transform that takes the box `from` to the box `to`: a scale about
/// `from`'s corner, then a move. A zero side is left alone, since nothing
/// scales a line's width into a width.
pub fn fit(from: Bounds, to: Bounds) -> Transform {
    let ratio = |to: f64, from: f64| if from == 0.0 { 1.0 } else { to / from };
    Transform::translate(-from.x, -from.y)
        .then(&Transform::scale(
            ratio(to.width, from.width),
            ratio(to.height, from.height),
        ))
        .then(&Transform::translate(to.x, to.y))
}

/// The geometry attributes of `shape` fitted to `to`, in the shape's own
/// coordinates. A line keeps its direction (which corner each end is at); a
/// circle takes the smaller side; a label moves its anchor and has no size.
/// `None` for a `<path>` or a `<g>`, which fit by [`fitted`], and for a
/// points list that does not parse.
pub fn resized(shape: &Shape, to: Bounds) -> Option<Vec<Update>> {
    let f = |v: f64| Some(number::fmt(v));
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
        // A label keeps its size; its anchor goes where its box's corner
        // went.
        ShapeKind::Text => {
            let from = bounds(shape)?;
            return moved(shape, to.x - from.x, to.y - from.y);
        }
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
                Some(fmt_points(pts.iter().map(|(x, y)| {
                    (to.x + (x - from.x) * sx, to.y + (y - from.y) * sy)
                }))),
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
            text: (kind == ShapeKind::Text).then(|| "Hello".to_string()),
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
    fn a_label_has_a_nominal_box_around_its_anchor() {
        // "Hello" at 20: 60 wide, 16 above the baseline, 4 below.
        let b = |a: &[(&str, &str)]| bounds(&shape(ShapeKind::Text, a)).unwrap();
        let start = b(&[("x", "10"), ("y", "50"), ("font-size", "20")]);
        assert_eq!(
            start,
            Bounds {
                x: 10.0,
                y: 34.0,
                width: 60.0,
                height: 20.0
            }
        );
        let end = b(&[
            ("x", "10"),
            ("y", "50"),
            ("font-size", "20px"),
            ("text-anchor", "end"),
        ]);
        assert_eq!(end.x, -50.0);
        let middle = b(&[("x", "10"), ("y", "50"), ("text-anchor", "middle")]);
        assert_eq!((middle.x, middle.width, middle.height), (-8.0, 36.0, 12.0));
        // Resizing moves the anchor by where the box's corner went.
        let moved_to = resized(
            &shape(ShapeKind::Text, &[("x", "10"), ("y", "50")]),
            Bounds {
                x: 20.0,
                y: 50.0,
                width: 1.0,
                height: 1.0,
            },
        )
        .unwrap();
        assert_eq!(
            moved_to,
            [
                ("x", Some("20".to_string())),
                ("y", Some("59.6".to_string()))
            ]
        );
    }

    #[test]
    fn moved_shifts_only_the_position() {
        let m = |k, a: &[(&str, &str)]| moved(&shape(k, a), 1.5, -2.0).unwrap();
        assert_eq!(
            m(ShapeKind::Rect, &[("x", "1"), ("width", "3")]),
            [
                ("x", Some("2.5".to_string())),
                ("y", Some("-2".to_string()))
            ]
        );
        assert_eq!(
            m(ShapeKind::Circle, &[("cx", "5"), ("cy", "5"), ("r", "2")]),
            [
                ("cx", Some("6.5".to_string())),
                ("cy", Some("3".to_string()))
            ]
        );
        assert_eq!(
            m(
                ShapeKind::Line,
                &[("x1", "0"), ("y1", "0"), ("x2", "4"), ("y2", "4")]
            ),
            [
                ("x1", Some("1.5".to_string())),
                ("x2", Some("5.5".to_string())),
                ("y1", Some("-2".to_string())),
                ("y2", Some("2".to_string()))
            ]
        );
        assert_eq!(
            m(ShapeKind::Polyline, &[("points", "0 0, 4,0")]),
            [("points", Some("1.5,-2 5.5,-2".to_string()))]
        );
        assert_eq!(
            m(ShapeKind::Group, &[]),
            [("transform", Some("translate(1.5 -2)".to_string()))]
        );
        // A path bakes the move, and whatever transform it carried, into
        // its `d`; the attribute comes off.
        assert_eq!(
            m(
                ShapeKind::Path,
                &[("d", "M0 0 h2"), ("transform", "translate(-1.5 2)")]
            ),
            [("d", Some("M0 0 L2 0".to_string())), ("transform", None)]
        );
        assert_eq!(
            m(
                ShapeKind::Path,
                &[("d", "M0 0 h2"), ("transform", "scale(2)")]
            ),
            [
                ("d", Some("M1.5 -2 L5.5 -2".to_string())),
                ("transform", None)
            ],
            "the delta is in the parent's space, so it is not scaled"
        );
        assert_eq!(
            m(
                ShapeKind::Path,
                &[("d", "M0 0 h2"), ("transform", "rotate(90)")]
            ),
            [("transform", Some("matrix(0 1 -1 0 1.5 -2)".to_string()))],
            "a rotation has nowhere to bake"
        );
    }

    #[test]
    fn baked_puts_an_axis_aligned_transform_into_the_attributes() {
        let t = Transform::scale(2.0, 0.5).then(&Transform::translate(10.0, 10.0));
        let b = |k, a: &[(&str, &str)]| baked(&shape(k, a), &t);
        let s = |v: &str| Some(v.to_string());
        assert_eq!(
            b(
                ShapeKind::Rect,
                &[
                    ("x", "1"),
                    ("y", "2"),
                    ("width", "3"),
                    ("height", "4"),
                    ("rx", "1")
                ]
            )
            .unwrap(),
            [
                ("x", s("12")),
                ("y", s("11")),
                ("width", s("6")),
                ("height", s("2")),
                ("transform", None)
            ],
            "a corner's radius is kept, as a stroke's width is"
        );
        assert_eq!(
            b(
                ShapeKind::Ellipse,
                &[("cx", "5"), ("cy", "5"), ("rx", "2"), ("ry", "2")]
            )
            .unwrap(),
            [
                ("cx", s("20")),
                ("cy", s("12.5")),
                ("rx", s("4")),
                ("ry", s("1")),
                ("transform", None)
            ]
        );
        assert!(
            b(ShapeKind::Circle, &[("cx", "5"), ("cy", "5"), ("r", "2")]).is_none(),
            "a circle under an unequal scale would be an ellipse"
        );
        assert!(
            b(ShapeKind::Text, &[("x", "5"), ("y", "5")]).is_none(),
            "a label under an unequal scale keeps its transform"
        );
        assert_eq!(
            b(ShapeKind::Polygon, &[("points", "0,0 1,1")]).unwrap(),
            [("points", s("10,10 12,10.5")), ("transform", None)]
        );
        let even = Transform::scale(2.0, 2.0);
        assert_eq!(
            baked(
                &shape(
                    ShapeKind::Text,
                    &[("x", "5"), ("y", "5"), ("font-size", "10")]
                ),
                &even
            )
            .unwrap(),
            [
                ("x", s("10")),
                ("y", s("10")),
                ("font-size", s("20")),
                ("transform", None)
            ]
        );
        assert!(
            baked(&shape(ShapeKind::Text, &[("x", "5")]), &even).is_none(),
            "no font-size of its own to scale"
        );
        // A flip normalizes a box and mirrors a path.
        let flip = Transform::scale(-1.0, 1.0);
        assert_eq!(
            baked(
                &shape(ShapeKind::Rect, &[("width", "3"), ("height", "4")]),
                &flip
            )
            .unwrap()[..2],
            [("x", s("-3")), ("y", s("0"))]
        );
        assert_eq!(
            baked(&shape(ShapeKind::Path, &[("d", "M1 0 L2 0")]), &flip).unwrap(),
            [("d", s("M-1 0 L-2 0")), ("transform", None)]
        );
        assert!(baked(&shape(ShapeKind::Group, &[]), &even).is_none());
        assert!(
            baked(
                &shape(ShapeKind::Rect, &[]),
                &Transform::parse("rotate(45)").unwrap()
            )
            .is_none()
        );
    }

    #[test]
    fn bounds_of_a_path_are_its_flattened_extent_and_a_group_has_none() {
        assert_eq!(
            bounds(&shape(ShapeKind::Path, &[("d", "M10 10 h20 v5 z")])),
            Some(Bounds {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 5.0
            })
        );
        assert!(bounds(&shape(ShapeKind::Path, &[("d", "nope")])).is_none());
        assert!(bounds(&shape(ShapeKind::Group, &[])).is_none());
        assert!(outline(&shape(ShapeKind::Group, &[])).is_none());
    }

    #[test]
    fn a_transformed_extent_is_around_the_outline() {
        let t = Transform::parse("rotate(90)").unwrap();
        let b = Bounds {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 4.0,
        };
        let r = b.transformed(&t);
        assert!((r.x + 4.0).abs() < 1e-9 && r.y.abs() < 1e-9);
        assert!((r.width - 4.0).abs() < 1e-9 && (r.height - 10.0).abs() < 1e-9);
        assert_eq!(
            b.union(&Bounds {
                x: 5.0,
                y: -1.0,
                width: 10.0,
                height: 1.0
            }),
            Bounds {
                x: 0.0,
                y: -1.0,
                width: 15.0,
                height: 5.0
            }
        );
        let circle = shape(ShapeKind::Circle, &[("cx", "0"), ("cy", "0"), ("r", "1")]);
        let pts = outline(&circle).unwrap().remove(0).points;
        assert_eq!(pts.len(), 48);
        assert!(pts.iter().all(|(x, y)| (x * x + y * y - 1.0).abs() < 1e-9));
    }

    #[test]
    fn fit_scales_about_the_old_corner() {
        let from = Bounds {
            x: 10.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        };
        let to = Bounds {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 5.0,
        };
        let t = fit(from, to);
        assert_eq!(t.apply(10.0, 10.0), (0.0, 0.0));
        assert_eq!(t.apply(20.0, 20.0), (20.0, 5.0));
        assert!(fit(from, from).is_identity());
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
                ("cx", Some("14".to_string())),
                ("cy", Some("22".to_string())),
                ("rx", Some("4".to_string())),
                ("ry", Some("2".to_string()))
            ]
        );
        assert_eq!(
            r(ShapeKind::Circle, &[("r", "1")]),
            [
                ("cx", Some("12".to_string())),
                ("cy", Some("22".to_string())),
                ("r", Some("2".to_string()))
            ]
        );
        assert_eq!(
            r(
                ShapeKind::Line,
                &[("x1", "9"), ("y1", "0"), ("x2", "0"), ("y2", "9")]
            ),
            [
                ("x1", Some("18".to_string())),
                ("y1", Some("20".to_string())),
                ("x2", Some("10".to_string())),
                ("y2", Some("24".to_string()))
            ]
        );
        assert_eq!(
            r(ShapeKind::Polygon, &[("points", "0,0 4,0 4,3")]),
            [("points", Some("10,20 18,20 18,24".to_string()))]
        );
    }
}
