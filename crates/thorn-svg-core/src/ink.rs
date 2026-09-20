//! Freehand ink: a stroke's centreline and widths in, the outline a nib
//! makes of them out. The outline is what the file draws — a `<path>`
//! whose `d` is filled, right in any viewer — and the centreline and
//! widths ride beside it (`data-centreline`, `data-widths`) so the outline
//! can be computed again: by this nib after a resize, by a different one
//! on a platform with a better idea of the pen. One writer, whatever the
//! stroke came from; a host that computed its own outline (PencilKit's)
//! hands that over as input rather than writing the file itself.
//!
//! The one nib so far is **monoline** — a round nib at one width, what a
//! mouse can draw. The profile admits only the nibs this module draws, so
//! the vocabulary grows with the rules and not ahead of them.

use crate::number;
use crate::path;

/// A nib: the rule that turns a centreline and its widths into an outline.
/// The value `data-ink` takes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum Nib {
    /// A round nib at one width, whatever the pressure.
    Monoline,
}

impl Nib {
    /// The value `data-ink` spells this as.
    pub fn value(self) -> &'static str {
        match self {
            Self::Monoline => "monoline",
        }
    }

    /// The nib a `data-ink` value names, or `None` for one this crate
    /// cannot draw.
    pub fn from_value(value: &str) -> Option<Self> {
        match value.trim() {
            "monoline" => Some(Self::Monoline),
            _ => None,
        }
    }
}

/// The centreline as `data-centreline` spells it: `M x y L x y …` in the
/// number format — a path `d`, so a host's cubics fit the same attribute.
pub fn centreline(points: &[(f64, f64)]) -> String {
    let mut out = String::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push(if i == 0 { 'M' } else { 'L' });
        out.push_str(&number::fmt(x));
        out.push(' ');
        out.push_str(&number::fmt(y));
    }
    out
}

/// The points along a `data-centreline`: its `d` flattened, curves
/// sampled. `None` when it does not parse.
pub fn centreline_points(d: &str) -> Option<Vec<(f64, f64)>> {
    let subs = path::flatten(d)?;
    Some(subs.into_iter().flat_map(|s| s.points).collect())
}

/// The widths as `data-widths` spells them: one per point, space
/// separated, in the number format. One value alone is the width at every
/// point, which is all a monoline stroke has to say.
pub fn widths(widths: &[f64]) -> String {
    widths
        .iter()
        .map(|&w| number::fmt(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The widths a `data-widths` value lists. `None` when one does not parse.
pub fn parse_widths(text: &str) -> Option<Vec<f64>> {
    text.split_whitespace()
        .map(|w| w.parse::<f64>().ok())
        .collect()
}

/// The width at point `i` of a stroke whose `widths` may be one value for
/// all, one per point, or short.
fn width_at(widths: &[f64], i: usize) -> f64 {
    widths
        .get(i)
        .or_else(|| widths.last())
        .copied()
        .unwrap_or(1.0)
}

/// The outline `d` of a stroke: what `nib` makes of `points` and
/// `widths`, as a closed path in the number format, filled. `None` for a
/// stroke with no points or no width.
pub fn outline(points: &[(f64, f64)], widths: &[f64], nib: Nib) -> Option<String> {
    match nib {
        Nib::Monoline => monoline(points, width_at(widths, 0)),
    }
}

/// A round nib of one width along the centreline: the two sides offset by
/// the half-width along each point's normal — the mean of its two
/// segments' directions, so a corner is mitred rather than split — and a
/// semicircle cap at each end.
fn monoline(points: &[(f64, f64)], width: f64) -> Option<String> {
    // A NaN width is no width either.
    if width.is_nan() || width <= 0.0 {
        return None;
    }
    let r = width / 2.0;
    // Consecutive points closer than the format can tell apart are one.
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for &p in points {
        if pts.last().is_none_or(|&q| dist(p, q) > 1e-3) {
            pts.push(p);
        }
    }
    let f = number::fmt;
    let pt = |(x, y): (f64, f64)| format!("{} {}", f(x), f(y));
    match pts.as_slice() {
        [] => return None,
        // A dot: a circle of the nib.
        &[(x, y)] => {
            let rr = f(r);
            return Some(format!(
                "M{} A{rr} {rr} 0 1 0 {} A{rr} {rr} 0 1 0 {} Z",
                pt((x - r, y)),
                pt((x + r, y)),
                pt((x - r, y))
            ));
        }
        _ => {}
    }
    let n = pts.len();
    let dir = |a: (f64, f64), b: (f64, f64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        (dx / len, dy / len)
    };
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        let t = if i == 0 {
            dir(pts[0], pts[1])
        } else if i == n - 1 {
            dir(pts[n - 2], pts[n - 1])
        } else {
            let a = dir(pts[i - 1], pts[i]);
            let b = dir(pts[i], pts[i + 1]);
            let (sx, sy) = (a.0 + b.0, a.1 + b.1);
            let len = (sx * sx + sy * sy).sqrt();
            // A reversal has no mean direction; the incoming one serves.
            if len < 1e-9 { a } else { (sx / len, sy / len) }
        };
        let (nx, ny) = (-t.1, t.0);
        let (x, y) = pts[i];
        left.push((x + nx * r, y + ny * r));
        right.push((x - nx * r, y - ny * r));
    }
    let rr = f(r);
    let mut d = String::new();
    d.push('M');
    d.push_str(&pt(left[0]));
    for &p in &left[1..] {
        d.push_str(" L");
        d.push_str(&pt(p));
    }
    // The end cap, round the front of the last point to the other side.
    d.push_str(&format!(" A{rr} {rr} 0 0 0 {}", pt(right[n - 1])));
    for &p in right[..n - 1].iter().rev() {
        d.push_str(" L");
        d.push_str(&pt(p));
    }
    // The start cap, round the back of the first point, closes it.
    d.push_str(&format!(" A{rr} {rr} 0 0 0 {} Z", pt(left[0])));
    Some(d)
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Bounds;

    fn bounds_of(d: &str) -> Bounds {
        let pts: Vec<(f64, f64)> = path::flatten(d)
            .unwrap()
            .into_iter()
            .flat_map(|s| s.points)
            .collect();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in pts {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        Bounds {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.05
    }

    #[test]
    fn the_centreline_and_widths_round_trip() {
        let pts = [(10.0, 10.5), (20.0, 10.0), (30.25, 12.0)];
        let d = centreline(&pts);
        assert_eq!(d, "M10 10.5 L20 10 L30.25 12");
        assert_eq!(centreline_points(&d).unwrap(), pts);
        assert_eq!(widths(&[4.0]), "4");
        assert_eq!(widths(&[2.0, 2.5, 3.0]), "2 2.5 3");
        assert_eq!(parse_widths("2 2.5 3").unwrap(), [2.0, 2.5, 3.0]);
        assert!(parse_widths("2 x").is_none());
    }

    #[test]
    fn a_monoline_outline_is_the_stroke_plus_half_a_width_all_round() {
        // A horizontal stroke of width 4: the outline runs from 2 before
        // its start to 2 past its end, and 2 either side — so the caps
        // bulge outwards, not back over the stroke.
        let d = outline(&[(10.0, 10.0), (50.0, 10.0)], &[4.0], Nib::Monoline).unwrap();
        let b = bounds_of(&d);
        assert!(close(b.x, 8.0) && close(b.width, 44.0), "{d}: {b:?}");
        assert!(close(b.y, 8.0) && close(b.height, 4.0), "{d}: {b:?}");
        assert!(d.ends_with(" Z"));

        // A dot is a circle of the nib.
        let dot = outline(&[(5.0, 5.0)], &[2.0], Nib::Monoline).unwrap();
        let b = bounds_of(&dot);
        assert!(
            close(b.x, 4.0) && close(b.width, 2.0) && close(b.height, 2.0),
            "{dot}"
        );

        // A right angle: one outline, both arms covered, no gap at the
        // corner — the sides through it are offset along the mean normal.
        let bent = outline(
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
            &[2.0],
            Nib::Monoline,
        )
        .unwrap();
        let b = bounds_of(&bent);
        assert!(close(b.x, -1.0) && close(b.y, -1.0), "{bent}: {b:?}");
        assert!(
            close(b.width, 12.0) && close(b.height, 12.0),
            "{bent}: {b:?}"
        );
        assert_eq!(bent.matches('M').count(), 1);

        assert!(outline(&[], &[2.0], Nib::Monoline).is_none());
        assert!(outline(&[(0.0, 0.0)], &[0.0], Nib::Monoline).is_none());
    }

    #[test]
    fn the_nib_vocabulary_is_what_this_draws() {
        assert_eq!(Nib::from_value("monoline"), Some(Nib::Monoline));
        assert_eq!(Nib::Monoline.value(), "monoline");
        assert_eq!(Nib::from_value("pen"), None);
    }
}
