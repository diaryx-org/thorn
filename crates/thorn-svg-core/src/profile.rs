//! The Diaryx drawing profile, held to a file. What the profile *is* is
//! written in `docs/profile.md`; this is the same rules as code, one
//! [`Rule`] per rule, and [`check`] is what `thorn check` runs.
//!
//! Every rule has the same fallback: a reader that knows nothing but SVG gets
//! the correct silhouette in flat colour. So none of these makes a file
//! unreadable — a finding is a promise the editor's output makes that this
//! file does not keep.

use std::collections::HashMap;

use crate::drawing::Drawing;
use crate::number;

/// The profile's version, written as the root's `data-diaryx-drawing` value.
pub const VERSION: &str = "1";

/// The marker attribute on `<svg>` that says a file follows the profile.
pub const MARKER: &str = "data-diaryx-drawing";

/// The attribute every shape carries, by which the editor addresses it.
pub const ID: &str = "data-id";

/// One rule of the profile, as `docs/profile.md` numbers them.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum Rule {
    /// `<svg>` carries `data-diaryx-drawing`, with the profile version as its
    /// value.
    Marker,
    /// `<svg>` carries `viewBox`: the drawing is page-shaped, and an embed
    /// and a site can ask its size without laying it out.
    ViewBox,
    /// Every shape carries `data-id`.
    ShapeId,
    /// No two shapes carry the same `data-id`.
    UniqueId,
    /// Every geometry attribute is a number in the profile's format — at most
    /// three decimals, no trailing zeros — so an unedited re-export is
    /// byte-stable.
    NumberFormat,
}

impl Rule {
    /// The rule in one line, for a report.
    pub fn about(self) -> &'static str {
        match self {
            Self::Marker => "<svg> carries data-diaryx-drawing with the profile version",
            Self::ViewBox => "<svg> carries viewBox",
            Self::ShapeId => "every shape carries data-id",
            Self::UniqueId => "no two shapes share a data-id",
            Self::NumberFormat => {
                "every geometry attribute is a number with at most three decimals and no trailing zeros"
            }
        }
    }
}

/// One way a file breaks the profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub rule: Rule,
    /// The shape's `data-id`, when the finding is about one shape that has
    /// one; `None` for a finding about the root or an unidentified shape.
    pub shape: Option<String>,
    /// What was found, for a person.
    pub message: String,
}

/// Hold a drawing to the profile. Empty means it conforms.
pub fn check(drawing: &Drawing) -> Vec<Finding> {
    let mut findings = Vec::new();
    let root = |name: &str| {
        drawing
            .root_attrs()
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.as_deref())
    };

    match root(MARKER) {
        Some(v) if v == VERSION => {}
        Some(v) => findings.push(Finding {
            rule: Rule::Marker,
            shape: None,
            message: format!("{MARKER} is {v:?}; this profile is version {VERSION:?}"),
        }),
        None => findings.push(Finding {
            rule: Rule::Marker,
            shape: None,
            message: format!("<svg> has no {MARKER} attribute"),
        }),
    }
    if root("viewBox").is_none() {
        findings.push(Finding {
            rule: Rule::ViewBox,
            shape: None,
            message: "<svg> has no viewBox attribute".to_string(),
        });
    }

    let mut seen: HashMap<&str, usize> = HashMap::new();
    for (index, shape) in drawing.shapes().iter().enumerate() {
        let tag = shape.kind.tag();
        match shape.id.as_deref() {
            Some(id) => {
                if let Some(first) = seen.insert(id, index) {
                    findings.push(Finding {
                        rule: Rule::UniqueId,
                        shape: Some(id.to_string()),
                        message: format!(
                            "<{tag}> (shape {index}) repeats {ID}={id:?} of shape {first}"
                        ),
                    });
                }
            }
            None => findings.push(Finding {
                rule: Rule::ShapeId,
                shape: None,
                message: format!("<{tag}> (shape {index}) has no {ID}"),
            }),
        }
        for &name in shape.kind.geometry_attrs() {
            let Some(value) = shape.attr(name) else {
                continue;
            };
            if !number::is_canonical(value) {
                findings.push(Finding {
                    rule: Rule::NumberFormat,
                    shape: shape.id.clone(),
                    message: format!(
                        "<{tag}> {name}={value:?} is not in the profile's number format{}",
                        value
                            .parse::<f64>()
                            .map(|v| format!(" (would be written {:?})", number::fmt(v)))
                            .unwrap_or_default()
                    ),
                });
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(src: &str) -> Vec<Rule> {
        let mut rules = check(&Drawing::open(src).unwrap())
            .into_iter()
            .map(|f| f.rule)
            .collect::<Vec<_>>();
        rules.dedup();
        rules
    }

    #[test]
    fn a_conforming_file_has_no_findings() {
        assert_eq!(
            rules(
                "<svg viewBox=\"0 0 9 9\" data-diaryx-drawing=\"1\"><rect x=\"1\" y=\"1.5\" width=\"2\" height=\"2\" data-id=\"a\"/><text x=\"1\" y=\"1\" data-id=\"b\">hi</text></svg>"
            ),
            []
        );
    }

    #[test]
    fn each_rule_is_reported_once_per_breach() {
        assert_eq!(rules("<svg/>"), [Rule::Marker, Rule::ViewBox]);
        assert_eq!(
            rules("<svg viewBox=\"0 0 1 1\" data-diaryx-drawing=\"2\"/>"),
            [Rule::Marker]
        );
        assert_eq!(
            rules(
                "<svg viewBox=\"0 0 1 1\" data-diaryx-drawing=\"1\"><rect x=\"1\"/><circle cx=\"1\" data-id=\"a\"/><circle cx=\"2\" data-id=\"a\"/></svg>"
            ),
            [Rule::ShapeId, Rule::UniqueId]
        );
        let findings = check(&Drawing::open("<svg viewBox=\"0 0 1 1\" data-diaryx-drawing=\"1\"><rect x=\"1.0\" y=\"1.2345\" width=\"2\" height=\"2\" data-id=\"a\"/></svg>").unwrap());
        assert_eq!(findings.len(), 2);
        assert!(
            findings
                .iter()
                .all(|f| f.rule == Rule::NumberFormat && f.shape.as_deref() == Some("a"))
        );
        assert!(findings[1].message.contains("would be written \"1.235\""));
    }

    /// Elements the profile does not name are neither shapes nor findings:
    /// they are what the editor preserves.
    #[test]
    fn noise_is_preserved_not_judged() {
        assert_eq!(
            rules(
                "<svg viewBox=\"0 0 1 1\" data-diaryx-drawing=\"1\"><defs><marker id=\"m\"/></defs><style>.a{}</style><title>t</title></svg>"
            ),
            []
        );
    }
}
