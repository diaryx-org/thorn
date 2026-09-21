//! The drawing's `<style>`, read just far enough to know what it strokes.
//!
//! A bent connector is a `<path>`, and how it looks is the drawing's own
//! stylesheet's to say. A drawing made before the template styled `path`
//! says nothing about one, and SVG's defaults — a black fill, no stroke,
//! no marker — turn a bent arrow into a silhouette. [`widened_for_paths`]
//! is the repair: each rule that selects a `line` comes to select the
//! `path` twin too, exactly as the template's rules changed when a bend
//! became possible, and a rule that strokes gains `fill: none` so the
//! path is an outline. A stylesheet that already has a rule for a bare
//! `path` is left alone: its author has said what a path is.
//!
//! This is not a CSS parser. It walks top-level `selector { declarations }`
//! blocks, skips comments and any `@`-rule block whole, and never reads
//! inside a declaration beyond asking whether a property is named.

use std::ops::Range;

/// A top-level rule: where its selector list and its declarations are in
/// the stylesheet text.
#[derive(Debug, PartialEq)]
struct Rule {
    selectors: Range<usize>,
    declarations: Range<usize>,
}

/// The top-level rules of `css`, in order. An `@media` (or any at-rule)
/// block is skipped whole, comments are skipped, and text that never
/// closes is dropped.
fn rules(css: &str) -> Vec<Rule> {
    let bytes = css.as_bytes();
    let mut rules = Vec::new();
    let mut i = 0;
    let mut prelude_start = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                // A comment before a selector list is not part of it.
                let leading = css[prelude_start..i].trim().is_empty();
                i = css[i + 2..]
                    .find("*/")
                    .map(|n| i + 2 + n + 2)
                    .unwrap_or(bytes.len());
                if leading {
                    prelude_start = i;
                }
            }
            b'{' => {
                let prelude = prelude_start..i;
                if css[prelude.clone()].trim_start().starts_with('@') {
                    // An at-rule's block may nest; skip to its close.
                    let mut depth = 0usize;
                    let mut j = i;
                    while j < bytes.len() {
                        match bytes[j] {
                            b'{' => depth += 1,
                            b'}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    i = j + 1;
                } else {
                    let Some(close) = css[i..].find('}') else {
                        break;
                    };
                    rules.push(Rule {
                        selectors: prelude,
                        declarations: i + 1..i + close,
                    });
                    i += close + 1;
                }
                prelude_start = i;
            }
            _ => i += 1,
        }
    }
    rules
}

/// The selectors of a list, trimmed, in order.
fn selectors(list: &str) -> impl Iterator<Item = &str> {
    list.split(',').map(str::trim).filter(|s| !s.is_empty())
}

/// Whether a selector is a `line` type selector, alone or qualified —
/// `line`, `line[data-arrow="end"]`, `line.thick`, `line:first-child`.
fn selects_line(selector: &str) -> bool {
    selector
        .strip_prefix("line")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['[', '.', ':', '#']))
}

/// The value a declaration block gives `property`, trimmed, if it names
/// it.
fn declared<'a>(declarations: &'a str, property: &str) -> Option<&'a str> {
    declarations.split(';').find_map(|d| {
        let (name, value) = d.split_once(':')?;
        (name.trim() == property).then(|| value.trim())
    })
}

/// Whether a declaration block names `property`.
fn declares(declarations: &str, property: &str) -> bool {
    declared(declarations, property).is_some()
}

/// `css` with every rule that selects a `line` selecting its `path` twin
/// too, and `fill: none` on each such rule that strokes and says nothing
/// of fill. `None` when nothing changes: no rule selects a `line`, or a
/// rule already selects a bare `path`.
pub(crate) fn widened_for_paths(css: &str) -> Option<String> {
    let rules = rules(css);
    if rules
        .iter()
        .any(|r| selectors(&css[r.selectors.clone()]).any(|s| s == "path"))
    {
        return None;
    }
    let mut out = String::with_capacity(css.len() + 64);
    let mut at = 0;
    let mut changed = false;
    let mut stroke = None;
    for rule in &rules {
        let list = &css[rule.selectors.clone()];
        let twins: Vec<String> = selectors(list)
            .filter(|s| selects_line(s))
            .map(|s| format!("path{}", &s["line".len()..]))
            .collect();
        if twins.is_empty() {
            continue;
        }
        changed = true;
        // The twins follow the list, before the whitespace that leads
        // into the brace: `line, path {` and `line[…], path[…] {`.
        let trimmed = list.trim_end();
        out.push_str(&css[at..rule.selectors.start + trimmed.len()]);
        for twin in &twins {
            out.push_str(", ");
            out.push_str(twin);
        }
        out.push_str(&list[trimmed.len()..]);
        out.push('{');
        let declarations = &css[rule.declarations.clone()];
        if stroke.is_none() {
            stroke = declared(declarations, "stroke");
        }
        if declares(declarations, "stroke") && !declares(declarations, "fill") {
            let lead = declarations.len() - declarations.trim_start().len();
            out.push_str(&declarations[..lead]);
            out.push_str("fill: none; ");
            out.push_str(&declarations[lead..]);
        } else {
            out.push_str(declarations);
        }
        at = rule.declarations.end;
    }
    if !changed {
        return None;
    }
    // The widened rule reaches the arrowhead's own `<path>` inside the
    // `<marker>` too, which was a filled triangle by SVG's default; a
    // rule of its own keeps it one, in the colour the line is stroked,
    // after the last rule widened and in its indentation, unless the
    // stylesheet has one already.
    out.push_str(&css[at..at + 1]);
    at += 1;
    if !rules
        .iter()
        .any(|r| selectors(&css[r.selectors.clone()]).any(|s| s == "marker path"))
    {
        let last = rules
            .iter()
            .rev()
            .find(|r| selectors(&css[r.selectors.clone()]).any(selects_line))
            .expect("a rule was widened");
        let list = &css[last.selectors.clone()];
        out.push_str(&list[..list.len() - list.trim_start().len()]);
        out.push_str(&marker_rule(stroke.unwrap_or("currentColor")));
    }
    out.push_str(&css[at..]);
    Some(out)
}

/// The rule that keeps an arrowhead a filled triangle once a bare `path`
/// is stroked, in `fill`: what the template says with `currentColor`,
/// and what widening adds in the line's own stroke.
fn marker_rule(fill: &str) -> String {
    format!("marker path {{ fill: {fill}; stroke: none }}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "\n    rect, ellipse, polygon { fill: none; stroke: #222; stroke-width: 2 }\n    line { stroke: #222; stroke-width: 2 }\n    line[data-arrow=\"end\"], line[data-arrow=\"both\"] { marker-end: url(#arrow) }\n    line[data-arrow=\"start\"], line[data-arrow=\"both\"] { marker-start: url(#arrow) }\n    path[data-ink] { fill: #222; stroke: none }\n    text { font: 16px sans-serif }\n  ";

    #[test]
    fn the_rules_are_found_and_at_rules_and_comments_skipped() {
        let css = "@media (prefers-color-scheme: dark) { svg { color: #eee } }\n/* a { b: c } */ line { stroke: red } text{}";
        let found: Vec<(&str, &str)> = rules(css)
            .iter()
            .map(|r| (&css[r.selectors.clone()], &css[r.declarations.clone()]))
            .collect();
        assert_eq!(
            found,
            vec![(" line ", " stroke: red "), (" text", "")],
            "{found:?}"
        );
    }

    #[test]
    fn the_old_template_is_widened_to_the_new_one() {
        let widened = widened_for_paths(OLD).unwrap();
        assert_eq!(
            widened,
            "\n    rect, ellipse, polygon { fill: none; stroke: #222; stroke-width: 2 }\n    line, path { fill: none; stroke: #222; stroke-width: 2 }\n    line[data-arrow=\"end\"], line[data-arrow=\"both\"], path[data-arrow=\"end\"], path[data-arrow=\"both\"] { marker-end: url(#arrow) }\n    line[data-arrow=\"start\"], line[data-arrow=\"both\"], path[data-arrow=\"start\"], path[data-arrow=\"both\"] { marker-start: url(#arrow) }\n    marker path { fill: #222; stroke: none }\n    path[data-ink] { fill: #222; stroke: none }\n    text { font: 16px sans-serif }\n  "
        );
        assert_eq!(widened_for_paths(&widened), None, "done once");
    }

    #[test]
    fn a_stylesheet_that_already_styles_a_path_or_no_line_is_left_alone() {
        assert_eq!(
            widened_for_paths("line { stroke: red }\npath { stroke: blue }"),
            None
        );
        assert_eq!(widened_for_paths("rect { fill: none }"), None);
        assert_eq!(widened_for_paths(""), None);
    }

    #[test]
    fn a_qualified_line_selector_gets_its_twin_and_fill_only_where_it_strokes() {
        assert_eq!(
            widened_for_paths("line.thick { stroke-width: 4 }\nline { fill: red; stroke: red }")
                .unwrap(),
            "line.thick, path.thick { stroke-width: 4 }\nline, path { fill: red; stroke: red }\nmarker path { fill: red; stroke: none }"
        );
        assert_eq!(
            widened_for_paths("linear { stroke: red }"),
            None,
            "`linear` is not `line`"
        );
    }
}
