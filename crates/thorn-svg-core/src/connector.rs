//! A connector: the line-like shape an arrow is. A `<line>`, or a `<path>`
//! whose `d` is one segment from one end to the other — straight (`M … L …`)
//! or bent through one control point (`M … Q … …`) — and which is not ink.
//! The binding gestures act on a connector whichever way it is spelled, and
//! a bend is what turns a `<line>` into the `<path>` and back.
//!
//! Which `<path>`s count is read off the `d`, not declared: a path a hand
//! wrote as `M 0 0 Q 50 80 100 0` is bendable on sight, as a `<line>` with
//! `marker-end` is an arrow on sight (`Shape::heads`).

use svgtypes::{SimplePathSegment as Seg, SimplifyingPathParser};

use crate::number;
use crate::shape::{Shape, ShapeKind};

/// An end of a connector: the coordinates it is at and the attribute that
/// binds it to a shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum End {
    /// `(x1, y1)`, or the path's `M`; bound by `data-from`.
    From,
    /// `(x2, y2)`, or the path's last point; bound by `data-to`.
    To,
}

impl End {
    /// The attribute that binds this end.
    pub fn binding(self) -> &'static str {
        match self {
            End::From => "data-from",
            End::To => "data-to",
        }
    }

    pub(crate) fn other(self) -> End {
        match self {
            End::From => End::To,
            End::To => End::From,
        }
    }
}

/// A connector's geometry, in the shape's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Connector {
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// The quadratic's control point; `None` for a straight one.
    pub control: Option<(f64, f64)>,
}

impl Connector {
    /// The connector `shape` is, or `None`: a `<line>` always; a `<path>`
    /// when it is not ink and its `d` is one `M` and one `L` or `Q`, in
    /// any spelling svgtypes simplifies to that (`l`, `H`, `T` …).
    pub fn of(shape: &Shape) -> Option<Self> {
        let n = |name: &str| shape.number(name).unwrap_or(0.0);
        match shape.kind {
            ShapeKind::Line => Some(Self {
                from: (n("x1"), n("y1")),
                to: (n("x2"), n("y2")),
                control: None,
            }),
            ShapeKind::Path if shape.attr("data-ink").is_none() => Self::parse(shape.attr("d")?),
            _ => None,
        }
    }

    fn parse(d: &str) -> Option<Self> {
        let mut segs = SimplifyingPathParser::from(d);
        let Seg::MoveTo { x, y } = segs.next()?.ok()? else {
            return None;
        };
        let from = (x, y);
        let (control, to) = match segs.next()?.ok()? {
            Seg::LineTo { x, y } => (None, (x, y)),
            Seg::Quadratic { x1, y1, x, y } => (Some((x1, y1)), (x, y)),
            _ => return None,
        };
        if segs.next().is_some() {
            return None;
        }
        Some(Self { from, to, control })
    }

    /// Whether this is bent: a `<path>` with a `Q`, not a `<line>`.
    pub fn is_bent(&self) -> bool {
        self.control.is_some()
    }

    /// Where an end is.
    pub fn end(&self, end: End) -> (f64, f64) {
        match end {
            End::From => self.from,
            End::To => self.to,
        }
    }

    /// Put an end somewhere. The control point stays, so a bent
    /// connector changes shape as its end moves — as it does in every
    /// drawing app.
    pub fn set_end(&mut self, end: End, at: (f64, f64)) {
        match end {
            End::From => self.from = at,
            End::To => self.to = at,
        }
    }

    /// The point half-way along: the chord's midpoint when straight, the
    /// curve's point at t = ½ when bent. Where the canvas puts the handle
    /// a bend is dragged by, so the handle sits on the stroke.
    pub fn midpoint(&self) -> (f64, f64) {
        let (a, b) = (self.from, self.to);
        match self.control {
            None => ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
            Some(c) => (
                0.25 * a.0 + 0.5 * c.0 + 0.25 * b.0,
                0.25 * a.1 + 0.5 * c.1 + 0.25 * b.1,
            ),
        }
    }

    /// This connector bent so that its midpoint is `p`: the control point
    /// a quadratic through the ends needs to pass through `p` at t = ½.
    pub fn through(&self, p: (f64, f64)) -> Self {
        let (a, b) = (self.from, self.to);
        Self {
            control: Some((2.0 * p.0 - (a.0 + b.0) / 2.0, 2.0 * p.1 - (a.1 + b.1) / 2.0)),
            ..*self
        }
    }

    /// This connector straightened.
    pub fn straight(&self) -> Self {
        Self {
            control: None,
            ..*self
        }
    }

    /// The point a bound end faces — what it is aimed at from its
    /// target's centre: the control point when bent, so the stroke leaves
    /// the shape along its tangent; otherwise `other`, the far end's
    /// anchor (its target's centre, or the end itself).
    pub fn facing(&self, other: (f64, f64)) -> (f64, f64) {
        self.control.unwrap_or(other)
    }

    /// The `d` a bent connector is written as, in the profile's number
    /// format and the spelling `path::transformed` writes, so a move that
    /// bakes into the `d` leaves the same bytes.
    pub fn d(&self) -> String {
        let p = |(x, y): (f64, f64)| format!("{} {}", number::fmt(x), number::fmt(y));
        match self.control {
            Some(c) => format!("M{} Q{} {}", p(self.from), p(c), p(self.to)),
            None => format!("M{} L{}", p(self.from), p(self.to)),
        }
    }

    /// A `<line>`'s four attributes for this geometry.
    pub fn line_attrs(&self) -> [(&'static str, String); 4] {
        [
            ("x1", number::fmt(self.from.0)),
            ("y1", number::fmt(self.from.1)),
            ("x2", number::fmt(self.to.0)),
            ("y2", number::fmt(self.to.1)),
        ]
    }
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
            text: None,
        }
    }

    #[test]
    fn a_line_and_a_one_segment_path_are_connectors() {
        let line = shape(
            ShapeKind::Line,
            &[("x1", "0"), ("y1", "0"), ("x2", "10"), ("y2", "0")],
        );
        let c = Connector::of(&line).unwrap();
        assert_eq!((c.from, c.to, c.control), ((0.0, 0.0), (10.0, 0.0), None));
        assert_eq!(c.midpoint(), (5.0, 0.0));

        let bent = shape(ShapeKind::Path, &[("d", "M 0 0 Q 5 8, 10 0")]);
        let c = Connector::of(&bent).unwrap();
        assert_eq!(c.control, Some((5.0, 8.0)));
        assert_eq!(c.midpoint(), (5.0, 4.0));
        assert_eq!(c.d(), "M0 0 Q5 8 10 0");

        // Relative and shorthand spellings simplify to the same.
        let hand = shape(ShapeKind::Path, &[("d", "m1 1 h9")]);
        assert_eq!(
            Connector::of(&hand).unwrap(),
            Connector {
                from: (1.0, 1.0),
                to: (10.0, 1.0),
                control: None
            }
        );
    }

    #[test]
    fn what_is_not_a_connector() {
        let two = shape(ShapeKind::Path, &[("d", "M0 0 L5 5 L10 0")]);
        assert!(Connector::of(&two).is_none(), "two segments");
        let cubic = shape(ShapeKind::Path, &[("d", "M0 0 C1 1 2 2 3 3")]);
        assert!(Connector::of(&cubic).is_none(), "a cubic");
        let closed = shape(ShapeKind::Path, &[("d", "M0 0 L10 0 Z")]);
        assert!(Connector::of(&closed).is_none(), "closed");
        let ink = shape(
            ShapeKind::Path,
            &[("d", "M0 0 L10 0"), ("data-ink", "monoline")],
        );
        assert!(Connector::of(&ink).is_none(), "ink");
        let rect = shape(ShapeKind::Rect, &[("x", "0")]);
        assert!(Connector::of(&rect).is_none());
    }

    #[test]
    fn bending_through_a_point_puts_the_midpoint_there() {
        let c = Connector {
            from: (0.0, 0.0),
            to: (10.0, 0.0),
            control: None,
        };
        let bent = c.through((5.0, 4.0));
        assert_eq!(bent.control, Some((5.0, 8.0)));
        assert_eq!(bent.midpoint(), (5.0, 4.0));
        assert_eq!(bent.straight(), c);
        assert_eq!(bent.facing((99.0, 99.0)), (5.0, 8.0));
        assert_eq!(c.facing((99.0, 99.0)), (99.0, 99.0));
    }
}
