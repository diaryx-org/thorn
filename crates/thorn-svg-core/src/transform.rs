//! An SVG `transform`, as one affine matrix: parsed from the attribute's
//! list by svgtypes, composed up the group chain, written back in the
//! profile's spelling. A `<path>` and a `<g>` have no position in their
//! attributes, so this is how they move; every other kind carries one only
//! when a hand wrote it, and then hit-testing has to honour it.

use crate::number;

/// The matrix `[a c e; b d f; 0 0 1]`, as SVG spells it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translate(tx: f64, ty: f64) -> Self {
        Transform {
            e: tx,
            f: ty,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Transform {
            a: sx,
            d: sy,
            ..Self::IDENTITY
        }
    }

    /// A `transform` attribute's value as one matrix. `None` when the list
    /// does not parse; an absent attribute is [`Transform::IDENTITY`], which
    /// is the caller's `unwrap_or_default`.
    pub fn parse(text: &str) -> Option<Self> {
        let t: svgtypes::Transform = text.parse().ok()?;
        Some(Transform {
            a: t.a,
            b: t.b,
            c: t.c,
            d: t.d,
            e: t.e,
            f: t.f,
        })
    }

    /// The transform that applies `self` first, then `outer` — a shape's own
    /// transform `then` its group's is the shape in the group's parent's
    /// space.
    pub fn then(&self, outer: &Transform) -> Transform {
        Transform {
            a: outer.a * self.a + outer.c * self.b,
            b: outer.b * self.a + outer.d * self.b,
            c: outer.a * self.c + outer.c * self.d,
            d: outer.b * self.c + outer.d * self.d,
            e: outer.a * self.e + outer.c * self.f + outer.e,
            f: outer.b * self.e + outer.d * self.f + outer.f,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// The linear part alone, for a vector rather than a point.
    pub fn apply_vector(&self, dx: f64, dy: f64) -> (f64, f64) {
        (self.a * dx + self.c * dy, self.b * dx + self.d * dy)
    }

    /// `None` for a singular matrix — `scale(0)` — which maps everything to
    /// a point and nothing back.
    pub fn inverse(&self) -> Option<Transform> {
        let det = self.a * self.d - self.b * self.c;
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Transform {
            a,
            b,
            c,
            d,
            e: -(a * self.e + c * self.f),
            f: -(b * self.e + d * self.f),
        })
    }

    /// How much a length grows, as the geometric mean of the axes — what a
    /// hit tolerance in the parent's units is in this shape's.
    pub fn length_scale(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// No rotation or skew: a box maps to a box.
    pub fn is_axis_aligned(&self) -> bool {
        self.b == 0.0 && self.c == 0.0
    }

    /// The attribute value the profile writes: `translate(x y)` when that is
    /// all it is, `matrix(a b c d e f)` otherwise, and `None` for the
    /// identity, which is written by leaving the attribute off.
    pub fn fmt(&self) -> Option<String> {
        let n = number::fmt;
        let rounded = Transform {
            a: n(self.a).parse().unwrap_or(self.a),
            b: n(self.b).parse().unwrap_or(self.b),
            c: n(self.c).parse().unwrap_or(self.c),
            d: n(self.d).parse().unwrap_or(self.d),
            e: n(self.e).parse().unwrap_or(self.e),
            f: n(self.f).parse().unwrap_or(self.f),
        };
        if rounded.is_identity() {
            None
        } else if rounded.a == 1.0 && rounded.b == 0.0 && rounded.c == 0.0 && rounded.d == 1.0 {
            Some(format!("translate({} {})", n(self.e), n(self.f)))
        } else {
            Some(format!(
                "matrix({} {} {} {} {} {})",
                n(self.a),
                n(self.b),
                n(self.c),
                n(self.d),
                n(self.e),
                n(self.f)
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_list_and_writes_the_short_form() {
        let t = Transform::parse("translate(10, 5) scale(2)").unwrap();
        assert_eq!(t.apply(1.0, 1.0), (12.0, 7.0));
        assert_eq!(t.fmt().as_deref(), Some("matrix(2 0 0 2 10 5)"));
        assert_eq!(
            Transform::translate(3.0, -4.5).fmt().as_deref(),
            Some("translate(3 -4.5)")
        );
        assert_eq!(Transform::IDENTITY.fmt(), None);
        assert_eq!(
            Transform::translate(0.0001, 0.0).fmt(),
            None,
            "an identity to three decimals is the identity"
        );
        assert!(Transform::parse("rotate(").is_none());
    }

    #[test]
    fn then_composes_inner_first_and_inverse_undoes() {
        let inner = Transform::scale(2.0, 2.0);
        let outer = Transform::translate(10.0, 0.0);
        let both = inner.then(&outer);
        assert_eq!(both.apply(1.0, 1.0), (12.0, 2.0));
        let back = both.inverse().unwrap();
        assert_eq!(back.apply(12.0, 2.0), (1.0, 1.0));
        assert_eq!(both.length_scale(), 2.0);
        assert!(Transform::scale(0.0, 1.0).inverse().is_none());
        assert!(both.is_axis_aligned());
        assert!(!Transform::parse("rotate(30)").unwrap().is_axis_aligned());
    }
}
