//! The drawing's `<style>`, read through [`crate::css`].
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
//! Every rule, selector and declaration is the parser's; what is spliced
//! here is spliced where it says, since fig's editor inserts a member only
//! after the last, and these edits put a declaration first and a rule
//! between two. A stylesheet that is not CSS is left as it is.

use crate::css::{self, Body, Rule};
use crate::shape::Hue;

/// The top-level rules of `css` that hold declarations — what a type
/// selector can be widened in and a word's rules sit beside. Nothing for
/// a stylesheet that does not parse.
fn rules(css: &str) -> Vec<Rule> {
    css::stylesheet(css)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| matches!(r.body, Body::Declarations { .. }))
        .collect()
}

/// Whether a selector is a `line` type selector, alone or qualified —
/// `line`, `line[data-arrow="end"]`, `line.thick`, `line:first-child`.
fn selects_line(selector: &str) -> bool {
    selector
        .strip_prefix("line")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['[', '.', ':', '#']))
}

/// The leading whitespace of the line `at` is on.
fn indent_of(css: &str, at: usize) -> &str {
    let line = css[..at].rfind('\n').map_or(0, |i| i + 1);
    let text = &css[line..];
    &text[..text.len() - text.trim_start().len()]
}

/// `css` with every rule that selects a `line` selecting its `path` twin
/// too, and `fill: none` on each such rule that strokes and says nothing
/// of fill. `None` when nothing changes: no rule selects a `line`, or a
/// rule already selects a bare `path`.
pub(crate) fn widened_for_paths(css: &str) -> Option<String> {
    let rules = rules(css);
    let list = |r: &Rule| css::selectors(&css[r.prelude.clone()]);
    if rules.iter().any(|r| list(r).contains(&"path")) {
        return None;
    }
    let mut out = String::with_capacity(css.len() + 64);
    let mut at = 0;
    let mut stroke = None;
    let mut last = None;
    for rule in &rules {
        let twins: Vec<String> = list(rule)
            .into_iter()
            .filter(|s| selects_line(s))
            .map(|s| format!("path{}", &s["line".len()..]))
            .collect();
        if twins.is_empty() {
            continue;
        }
        let Body::Declarations { open, close, .. } = rule.body else {
            unreachable!("only declaration blocks are kept");
        };
        // The twins follow the list, before what leads into the brace:
        // `line, path {` and `line[…], path[…] {`.
        out.push_str(&css[at..rule.prelude.end]);
        for twin in &twins {
            out.push_str(", ");
            out.push_str(twin);
        }
        out.push_str(&css[rule.prelude.end..=open]);
        if stroke.is_none() {
            stroke = rule.declared("stroke");
        }
        let block = &css[open + 1..close];
        if rule.declared("stroke").is_some() && rule.declared("fill").is_none() {
            let lead = block.len() - block.trim_start().len();
            out.push_str(&block[..lead]);
            out.push_str("fill: none; ");
            out.push_str(&block[lead..]);
        } else {
            out.push_str(block);
        }
        at = close;
        last = Some(rule);
    }
    let last = last?;
    // The widened rule reaches the arrowhead's own `<path>` inside the
    // `<marker>` too, which was a filled triangle by SVG's default; a
    // rule of its own keeps it one, in the colour the line is stroked,
    // after the last rule widened and in its indentation, unless the
    // stylesheet has one already.
    out.push_str(&css[at..at + 1]);
    at += 1;
    if !rules.iter().any(|r| list(r).contains(&"marker path")) {
        out.push('\n');
        out.push_str(indent_of(css, last.prelude.start));
        out.push_str(&marker_rule(stroke.unwrap_or("currentColor")));
    }
    out.push_str(&css[at..]);
    Some(out)
}

/// The rules the template draws a `data-dash` by: what a drawing made
/// before the word gains when a shape first takes it.
pub(crate) const DASH_RULES: &[&str] = &[
    "[data-dash=\"dashed\"] { stroke-dasharray: 8 6 }",
    "[data-dash=\"dotted\"] { stroke-dasharray: 1 5; stroke-linecap: round }",
];

/// The rules the template draws a `data-weight` by.
pub(crate) const WEIGHT_RULES: &[&str] = &[
    "[data-weight=\"thin\"] { stroke-width: 1 }",
    "[data-weight=\"bold\"] { stroke-width: 4 }",
];

/// The rules the template draws a `data-color` and a `data-fill` by, one
/// block per hue — the `color` the word sets, on the shape and on the
/// arrowhead marker that hue has; the tint a fill is; and which marker a
/// coloured arrow's ends take — and then the darker page's variants, in
/// one `@media` block. What a drawing made before the palette gains when
/// a shape first takes a hue.
pub(crate) fn color_rules() -> Vec<String> {
    let mut out = Vec::new();
    for hue in Hue::ALL {
        let name = hue.value();
        out.push(format!(
            "[data-color=\"{name}\"], #arrow-{name} {{ color: {} }}",
            hue.stroke(false)
        ));
        out.push(format!(
            "[data-fill=\"{name}\"] {{ fill: {} }}",
            hue.tint(false)
        ));
        out.push(format!(
            "[data-arrow=\"end\"][data-color=\"{name}\"], [data-arrow=\"both\"][data-color=\"{name}\"] {{ marker-end: url(#arrow-{name}) }}"
        ));
        out.push(format!(
            "[data-arrow=\"start\"][data-color=\"{name}\"], [data-arrow=\"both\"][data-color=\"{name}\"] {{ marker-start: url(#arrow-{name}) }}"
        ));
    }
    let mut dark = String::from("@media (prefers-color-scheme: dark) {");
    for hue in Hue::ALL {
        let name = hue.value();
        dark.push_str(&format!(
            "\n  [data-color=\"{name}\"], #arrow-{name} {{ color: {} }}",
            hue.stroke(true)
        ));
        dark.push_str(&format!(
            "\n  [data-fill=\"{name}\"] {{ fill: {} }}",
            hue.tint(true)
        ));
    }
    dark.push_str("\n}");
    out.push(dark);
    out
}

/// The `<marker>` an arrow of `hue` takes its head from: the template's
/// `#arrow` under another id, coloured by the rule that names it. The
/// inner line is indented one step past `indent`.
pub(crate) fn marker_markup(hue: Hue, indent: &str) -> String {
    format!(
        "<marker id=\"arrow-{}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto-start-reverse\">\n{indent}  <path d=\"M 0 0 L 10 5 L 0 10 z\"/>\n{indent}</marker>",
        hue.value()
    )
}

/// `css` with `rules` appended after its last rule, in that rule's
/// indentation (a rule of several lines has each indented), unless a
/// selector already mentions `word` — in which case its author has said
/// what the word means, and `None`. A stylesheet with no rules at all
/// takes them on lines of their own; one that is not CSS is left alone.
pub(crate) fn with_rules<S: AsRef<str>>(css: &str, word: &str, rules: &[S]) -> Option<String> {
    let found = css::stylesheet(css).ok()?;
    if mentions(css, &found, word) {
        return None;
    }
    let mut out = String::with_capacity(css.len() + 128);
    // Just past the last top-level rule, indented as the line its end is on.
    let (head, indent, tail) = match found.last() {
        Some(last) => {
            let end = last.span.end;
            (&css[..end], indent_of(css, end - 1), &css[end..])
        }
        None => (css.trim_end(), "", "\n"),
    };
    out.push_str(head);
    for rule in rules {
        out.push('\n');
        out.push_str(indent);
        out.push_str(&rule.as_ref().replace('\n', &format!("\n{indent}")));
    }
    out.push_str(tail);
    Some(out)
}

/// Whether any selector among `rules`, at any depth, mentions `word`.
fn mentions(css: &str, rules: &[Rule], word: &str) -> bool {
    rules.iter().any(|r| {
        css[r.prelude.clone()].contains(word)
            || match &r.body {
                Body::Rules { rules, .. } | Body::Declarations { rules, .. } => {
                    mentions(css, rules, word)
                }
                Body::Statement => false,
            }
    })
}

/// The rules a drawing keeps for a darker page — the body of every
/// `@media (prefers-color-scheme: dark)` block in `css`, joined — so an
/// editor drawing the file in dark mode can apply them itself: resvg does
/// not read `@media`. Empty when the stylesheet has none.
pub(crate) fn dark_rules(css: &str) -> String {
    fn gather(css: &str, rules: &[Rule], out: &mut String) {
        for rule in rules {
            let Body::Rules { inner, rules } = &rule.body else {
                continue;
            };
            let prelude: String = css[rule.prelude.clone()]
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if prelude == "@media(prefers-color-scheme:dark)" {
                out.push_str(&css[inner.clone()]);
                out.push('\n');
            } else {
                gather(css, rules, out);
            }
        }
    }
    let mut out = String::new();
    gather(css, &css::stylesheet(css).unwrap_or_default(), &mut out);
    out
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
            .map(|r| {
                let Body::Declarations { open, close, .. } = r.body else {
                    unreachable!()
                };
                (&css[r.prelude.clone()], &css[open + 1..close])
            })
            .collect();
        assert_eq!(
            found,
            vec![("line", " stroke: red "), ("text", "")],
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
    fn rules_for_a_word_are_added_once_in_the_last_rules_indentation() {
        let css = "\n    rect { fill: none }\n    text { font: 16px sans-serif }\n  ";
        let added = with_rules(css, "[data-dash", DASH_RULES).unwrap();
        assert_eq!(
            added,
            "\n    rect { fill: none }\n    text { font: 16px sans-serif }\n    [data-dash=\"dashed\"] { stroke-dasharray: 8 6 }\n    [data-dash=\"dotted\"] { stroke-dasharray: 1 5; stroke-linecap: round }\n  "
        );
        assert_eq!(with_rules(&added, "[data-dash", DASH_RULES), None, "once");
        assert_eq!(
            with_rules(
                "line.x[data-dash] { stroke: red }",
                "[data-dash",
                DASH_RULES
            ),
            None,
            "the author has said what the word means"
        );
        assert_eq!(
            with_rules("", "[data-dash", &["a { b: c }"]).unwrap(),
            "\na { b: c }\n"
        );
    }

    #[test]
    fn a_dark_block_is_hoisted_and_a_rule_of_several_lines_is_indented() {
        let css = "\n    @media (prefers-color-scheme: dark) { svg { color: #eee } }\n    rect { fill: none }\n    @media print { rect { fill: red } }\n  ";
        assert_eq!(dark_rules(css), "svg { color: #eee }\n");
        assert_eq!(dark_rules("rect { fill: none }"), "");
        let added = with_rules(
            css,
            "[data-color",
            &[
                "a { b: c }",
                "@media (prefers-color-scheme: dark) {\n  a { b: d }\n}",
            ],
        )
        .unwrap();
        assert_eq!(
            added,
            "\n    @media (prefers-color-scheme: dark) { svg { color: #eee } }\n    rect { fill: none }\n    @media print { rect { fill: red } }\n    a { b: c }\n    @media (prefers-color-scheme: dark) {\n      a { b: d }\n    }\n  "
        );
        assert_eq!(dark_rules(&added), "svg { color: #eee }\na { b: d }\n");
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
