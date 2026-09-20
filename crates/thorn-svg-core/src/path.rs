//! A `<path>`'s `d`, flattened: every subpath as a polyline, so that the
//! extent and the hit-test a `<polyline>` already has apply to it. svgtypes
//! does the parsing — relative to absolute, `H`/`V`/`S`/`T` expanded, an arc
//! turned into cubics — and this only samples the curves.

use svgtypes::{SimplePathSegment as Seg, SimplifyingPathParser};

/// One `M`…`Z` run of a path, as the points along it.
#[derive(Clone, Debug, PartialEq)]
pub struct Subpath {
    pub points: Vec<(f64, f64)>,
    /// Ended by `Z`: the last point joins the first, and the run has an
    /// interior a fill paints.
    pub closed: bool,
}

/// Samples per curve segment. A selection box and a click tolerance are
/// what this feeds; sixteen chords are within a hair of any stroke the
/// editor draws, and the count is not a rendering concern.
const STEPS: usize = 16;

/// Flatten `d`. `None` when it does not parse or draws nothing.
pub fn flatten(d: &str) -> Option<Vec<Subpath>> {
    let mut subpaths: Vec<Subpath> = Vec::new();
    let mut current: Option<Subpath> = None;
    let mut last = (0.0, 0.0);
    for seg in SimplifyingPathParser::from(d) {
        let seg = seg.ok()?;
        match seg {
            Seg::MoveTo { x, y } => {
                subpaths.extend(current.take());
                current = Some(Subpath {
                    points: vec![(x, y)],
                    closed: false,
                });
                last = (x, y);
            }
            Seg::LineTo { x, y } => {
                current.as_mut()?.points.push((x, y));
                last = (x, y);
            }
            Seg::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let sub = current.as_mut()?;
                let (x0, y0) = last;
                for i in 1..=STEPS {
                    let t = i as f64 / STEPS as f64;
                    let u = 1.0 - t;
                    let px = u * u * u * x0
                        + 3.0 * u * u * t * x1
                        + 3.0 * u * t * t * x2
                        + t * t * t * x;
                    let py = u * u * u * y0
                        + 3.0 * u * u * t * y1
                        + 3.0 * u * t * t * y2
                        + t * t * t * y;
                    sub.points.push((px, py));
                }
                last = (x, y);
            }
            Seg::Quadratic { x1, y1, x, y } => {
                let sub = current.as_mut()?;
                let (x0, y0) = last;
                for i in 1..=STEPS {
                    let t = i as f64 / STEPS as f64;
                    let u = 1.0 - t;
                    let px = u * u * x0 + 2.0 * u * t * x1 + t * t * x;
                    let py = u * u * y0 + 2.0 * u * t * y1 + t * t * y;
                    sub.points.push((px, py));
                }
                last = (x, y);
            }
            Seg::ClosePath => {
                if let Some(mut sub) = current.take() {
                    sub.closed = true;
                    // The next segment without an `M` starts where this one did.
                    last = sub.points[0];
                    let start = sub.points[0];
                    subpaths.push(sub);
                    current = Some(Subpath {
                        points: vec![start],
                        closed: false,
                    });
                }
            }
        }
    }
    subpaths.extend(current.take());
    // A lone `M` — from a `Z` that nothing followed — draws nothing.
    subpaths.retain(|s| s.points.len() > 1);
    (!subpaths.is_empty()).then_some(subpaths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_and_closes() {
        let subs = flatten("M0 0 h10 v10 z m 20 0 l 5 5").unwrap();
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].points, [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert!(subs[0].closed);
        assert_eq!(subs[1].points, [(20.0, 0.0), (25.0, 5.0)]);
        assert!(!subs[1].closed);
    }

    #[test]
    fn curves_and_arcs_are_sampled_through_their_ends() {
        let subs = flatten("M0 0 C 0 10, 10 10, 10 0").unwrap();
        let pts = &subs[0].points;
        assert_eq!(pts.len(), 1 + STEPS);
        assert_eq!(*pts.last().unwrap(), (10.0, 0.0));
        // The cubic's midpoint is at y = 7.5.
        assert!((pts[STEPS / 2].1 - 7.5).abs() < 1e-9);
        // A half-circle arc of radius 5 reaches 5 off its chord (svgtypes
        // makes cubics of it; the samples come within a hair).
        let arc = flatten("M0 0 A 5 5 0 0 1 10 0").unwrap();
        let reach = arc[0].points.iter().map(|p| p.1.abs()).fold(0.0, f64::max);
        assert!((reach - 5.0).abs() < 0.01, "{reach}");
        let end = *arc[0].points.last().unwrap();
        assert!((end.0 - 10.0).abs() < 1e-9 && end.1.abs() < 1e-9, "{end:?}");
    }

    #[test]
    fn what_does_not_draw_is_none() {
        assert!(flatten("").is_none());
        assert!(flatten("M 1 1").is_none());
        assert!(flatten("M 1 1 Z").is_none());
        assert!(flatten("L 1 1").is_none(), "no M first");
    }
}
