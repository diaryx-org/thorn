//! CSS as a fig language: a stylesheet and a `style` attribute read into
//! fig's node table, so that fig's editor makes every edit to them as a
//! splice that keeps the bytes it was not asked to change — the comments,
//! the spacing, a property the editor has never heard of.
//!
//! Two dialects of one language, [`Css`]:
//!
//! - **`css`**, a stylesheet — what a `<style>` holds. The root is a block
//!   of rules: each a selector (as written, a list included) over its
//!   declaration block, a flow mapping whose span is its braces. An
//!   at-rule with a block is its prelude (`@media (…)`) over the rules in
//!   it, or over declarations for one that holds them (`@font-face`); one
//!   with none (`@import url(a.css);`) is its name over the rest of its
//!   prelude. A declaration block may nest a rule, as CSS nesting does.
//! - **`css-declarations`**, a declaration list — what a `style` attribute
//!   holds. The root is the flow mapping, with no braces.
//!
//! A declaration is a property over its value, the value as written:
//! `!important` and all, since CSS gives a value no decoding a writer needs
//! undone. Comments are trivia: kept where they are, never in the tree.
//!
//! Partial by design, as fig's own runtime languages say of themselves. The
//! editor replaces, renames, inserts and deletes declarations in a block or
//! a list, on one line or several; and replaces, renames, inserts and
//! deletes rules at the root, a rule that shares its line with another
//! taking only its own bytes. Inserting into an at-rule's block of rules is
//! not supported: its members are separated by nothing but whitespace, and
//! the separator a dialect declares is `;`. A property or a selector that
//! repeats is in the tree each time; a path names the first.

use std::ops::Range;
use std::sync::OnceLock;

use fig::language::{
    CommentDelimiter, CommentStyle, Comments, Description, Dialect, Language, LanguageError,
    NodeKind, NodeRow, NodeTable, PrintOptions, RenderArgs, Renderer, Renderers, Splice, Syntax,
};
use fig::{Capabilities, Format, Span};

/// The language's name, and its stylesheet dialect's.
pub const STYLESHEET: &str = "css";
/// The declaration-list dialect: a `style` attribute.
pub const DECLARATIONS: &str = "css-declarations";

/// At-rules whose block holds declarations rather than rules. Any other
/// at-rule's block is read as rules.
const DECLARATION_AT_RULES: &[&str] = &[
    "@font-face",
    "@page",
    "@counter-style",
    "@property",
    "@font-palette-values",
    "@viewport",
];

/// The CSS language. Register it with [`formats`], which does so once per
/// process.
pub struct Css;

/// The two formats [`Css`] registers as: the stylesheet and the
/// declaration list, in that order.
#[derive(Clone, Copy, Debug)]
pub struct Formats {
    pub stylesheet: Format,
    pub declarations: Format,
}

/// [`Css`], registered with fig: once per process, since a registration
/// lives until exit. An error is fig's refusal, kept and returned to every
/// caller after the first.
pub fn formats() -> Result<Formats, String> {
    static FORMATS: OnceLock<Result<Formats, String>> = OnceLock::new();
    FORMATS
        .get_or_init(|| match fig::language::register(Css) {
            Ok(f) => Ok(Formats {
                stylesheet: f[0],
                declarations: f[1],
            }),
            Err(e) => Err(e.to_string()),
        })
        .clone()
}

impl Language for Css {
    fn describe(&self) -> Description {
        let mut syntax = Syntax::default();
        // `/* */` is the only comment, and the slashes scanner is the one
        // that walks it; under the default `#` one a line opening with an
        // id selector would read as a comment a delete takes with it.
        syntax.comments = Comments::new(
            CommentStyle::Slashes,
            Some(CommentDelimiter::pair("/*", "*/")),
            Some(CommentDelimiter::pair("/*", "*/")),
        );
        syntax.kv_sep = Some(": ".into());
        syntax.flow_entry_sep = ";".into();
        syntax.flow_map_pad = " ".into();
        syntax.flow_maps_only = true;
        syntax.empty_map_literal = Some("{}".into());
        syntax.indent_unit = "  ".into();

        let mut declarations = syntax.clone();
        declarations.flow_root = true;

        let mut d = Description::new(STYLESHEET);
        d.caps = Capabilities::new(true, true, true);
        d.syntax = Some(syntax);
        let mut sheet = Dialect::new(STYLESHEET);
        sheet.extensions = vec!["css".into()];
        sheet.splice = Splice::Raw;
        sheet.empty_doc_seed = Some(String::new());
        let mut list = Dialect::new(DECLARATIONS);
        list.splice = Splice::Raw;
        list.empty_doc_seed = Some(String::new());
        list.syntax = Some(declarations);
        d.dialects = vec![sheet, list];
        d.samples = vec![
            "rect { fill: none; stroke: #222 }\n".into(),
            "@media (prefers-color-scheme: dark) {\n  svg { color: #eee }\n}\n".into(),
        ];
        d.renderers = Renderers::default();
        d.renderers.entry = true;
        d
    }

    fn parse(&self, dialect: &str, input: &[u8]) -> Result<NodeTable, LanguageError> {
        let src = std::str::from_utf8(input).map_err(|_| LanguageError::new("not UTF-8"))?;
        let mut p = Parser {
            src,
            b: src.as_bytes(),
            t: NodeTable::new(),
        };
        let root =
            p.t.push(NodeRow::new(NodeKind::Mapping, None, span(0, src.len())));
        if dialect == DECLARATIONS {
            p.declarations(root, 0, src.len())?;
        } else {
            p.rules(root, 0, src.len())?;
        }
        Ok(p.t)
    }

    fn print(
        &self,
        dialect: &str,
        table: &NodeTable,
        _options: &PrintOptions,
    ) -> Result<Vec<u8>, LanguageError> {
        let rows = &table.rows;
        let Some(root) = rows.first() else {
            return Ok(Vec::new());
        };
        // A scalar root is a fragment — a value the editor splices — and is
        // spelled as its text.
        if root.kind != NodeKind::Mapping {
            return Ok(root.text.clone().unwrap_or_default().into_bytes());
        }
        let mut out = String::new();
        if dialect == DECLARATIONS {
            print_declarations(rows, 0, &mut out);
        } else {
            print_rules(rows, 0, "", &mut out);
        }
        Ok(out.into_bytes())
    }

    fn render(&self, which: Renderer, args: RenderArgs<'_>) -> Result<Vec<u8>, LanguageError> {
        match which {
            // A block entry is a rule: the selector, a space, the block. A
            // declaration is never a block entry — it goes into a flow
            // mapping, which the engine spells with `kv_sep` itself.
            Renderer::Entry => Ok([args.key, b" ", args.value].concat()),
            _ => Err(LanguageError::new(format!(
                "css does not render {}",
                which.name()
            ))),
        }
    }
}

fn span(start: usize, end: usize) -> Span {
    Span { start, end }
}

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    t: NodeTable,
}

impl Parser<'_> {
    /// Past whitespace and comments from `i`, not beyond `to`.
    fn trivia(&self, mut i: usize, to: usize) -> Result<usize, LanguageError> {
        loop {
            while i < to && self.b[i].is_ascii_whitespace() {
                i += 1;
            }
            if self.b[i..to].starts_with(b"/*") {
                i = self.comment_end(i, to)?;
            } else {
                return Ok(i);
            }
        }
    }

    fn comment_end(&self, at: usize, to: usize) -> Result<usize, LanguageError> {
        self.src[at + 2..to]
            .find("*/")
            .map(|e| at + 2 + e + 2)
            .ok_or_else(|| LanguageError::at("unclosed comment", at))
    }

    /// The first of `stops` at nesting depth zero from `i`, or `to`: past
    /// strings, comments, and bracketed runs, so a `;` in `url("a;b")` or a
    /// `{` in a custom property's value is not one. A closer with no opener
    /// is an error at it.
    fn scan(&self, mut i: usize, to: usize, stops: &[u8]) -> Result<usize, LanguageError> {
        let mut depth: Vec<u8> = Vec::new();
        while i < to {
            let c = self.b[i];
            if depth.is_empty() && stops.contains(&c) {
                return Ok(i);
            }
            match c {
                b'"' | b'\'' => i = self.string_end(i, to)?,
                b'/' if self.b[i..to].starts_with(b"/*") => i = self.comment_end(i, to)?,
                b'\\' => i += 2,
                b'(' | b'[' | b'{' => {
                    depth.push(c);
                    i += 1;
                }
                b')' | b']' | b'}' => {
                    let open = match c {
                        b')' => b'(',
                        b']' => b'[',
                        _ => b'{',
                    };
                    if depth.pop() != Some(open) {
                        return Err(LanguageError::at(format!("unbalanced `{}`", c as char), i));
                    }
                    i += 1;
                }
                _ => i += 1,
            }
        }
        if let Some(&open) = depth.last() {
            return Err(LanguageError::at(
                format!("unclosed `{}`", open as char),
                to,
            ));
        }
        Ok(to.min(i))
    }

    fn string_end(&self, at: usize, to: usize) -> Result<usize, LanguageError> {
        let quote = self.b[at];
        let mut i = at + 1;
        while i < to {
            match self.b[i] {
                b'\\' => i += 2,
                c if c == quote => return Ok(i + 1),
                b'\n' => break,
                _ => i += 1,
            }
        }
        Err(LanguageError::at("unclosed string", at))
    }

    /// The `}` closing the block opened at `open`.
    fn close_of(&self, open: usize, to: usize) -> Result<usize, LanguageError> {
        let close = self.scan(open + 1, to, b"}")?;
        if close >= to {
            return Err(LanguageError::at("unclosed `{`", open));
        }
        Ok(close)
    }

    /// `[from, to)` trimmed of whitespace and comments at its end, so a
    /// name or a value is its own bytes and not a remark after them.
    fn trim_end(&self, from: usize, mut to: usize) -> usize {
        loop {
            while to > from && self.b[to - 1].is_ascii_whitespace() {
                to -= 1;
            }
            match self.b[from..to]
                .ends_with(b"*/")
                .then(|| self.src[from..to - 2].rfind("/*"))
            {
                Some(Some(open)) => to = from + open,
                _ => return to,
            }
        }
    }

    fn string(&mut self, parent: u32, from: usize, to: usize) -> u32 {
        let row = NodeRow::new(NodeKind::String, Some(parent), span(from, to))
            .with_text(&self.src[from..to]);
        self.t.push(row)
    }

    /// The rules in `[from, to)`, under `parent`.
    fn rules(&mut self, parent: u32, from: usize, to: usize) -> Result<(), LanguageError> {
        let mut i = from;
        loop {
            i = self.trivia(i, to)?;
            // The HTML comment tokens a stylesheet may carry around itself.
            for cdx in [&b"<!--"[..], &b"-->"[..]] {
                if self.b[i..to].starts_with(cdx) {
                    i = self.trivia(i + cdx.len(), to)?;
                }
            }
            if i >= to {
                return Ok(());
            }
            let start = i;
            let stop = self.scan(i, to, b";{")?;
            let at_rule = self.b[start] == b'@';
            if stop >= to || self.b[stop] == b';' {
                if !at_rule {
                    return Err(LanguageError::at("expected `{` after a selector", start));
                }
                // `@import url(a.css);`: the at-rule's name over the rest.
                let name_end = start
                    + 1
                    + self.b[start + 1..stop]
                        .iter()
                        .position(|c| c.is_ascii_whitespace() || matches!(c, b'"' | b'\'' | b'('))
                        .unwrap_or(stop - start - 1);
                let end = if stop < to { stop + 1 } else { stop };
                let kv = self.t.push(NodeRow::new(
                    NodeKind::KeyValue,
                    Some(parent),
                    span(start, end),
                ));
                self.string(kv, start, name_end);
                let prelude = self.trivia(name_end, stop)?;
                let prelude_end = self.trim_end(prelude, stop);
                self.string(kv, prelude, prelude_end);
                i = end;
                continue;
            }
            let open = stop;
            let close = self.close_of(open, to)?;
            let kv = self.t.push(NodeRow::new(
                NodeKind::KeyValue,
                Some(parent),
                span(start, close + 1),
            ));
            let key_end = self.trim_end(start, open);
            self.string(kv, start, key_end);
            if at_rule && holds_rule_list(&self.src[start..key_end]) {
                // A block of rules: block, not flow — its span starts at its
                // first rule, so no `{` is sniffed — since its members are
                // separated by nothing a dialect could declare.
                let first = self.trivia(open + 1, close)?;
                let last = self.trim_end(first, close);
                let block =
                    self.t
                        .push(NodeRow::new(NodeKind::Mapping, Some(kv), span(first, last)));
                self.rules(block, open + 1, close)?;
            } else {
                let block = self.t.push(NodeRow::new(
                    NodeKind::Mapping,
                    Some(kv),
                    span(open, close + 1),
                ));
                self.declarations(block, open + 1, close)?;
            }
            i = close + 1;
        }
    }

    /// The declarations in `[from, to)`, under `parent` — and a nested
    /// rule, where one stands among them.
    fn declarations(&mut self, parent: u32, from: usize, to: usize) -> Result<(), LanguageError> {
        let mut i = from;
        loop {
            loop {
                i = self.trivia(i, to)?;
                if i < to && self.b[i] == b';' {
                    i += 1;
                } else {
                    break;
                }
            }
            if i >= to {
                return Ok(());
            }
            let start = i;
            let stop = self.scan(i, to, b":;{")?;
            if stop < to && self.b[stop] == b'{' {
                // A nested rule: a selector over a block of its own.
                let close = self.close_of(stop, to)?;
                let kv = self.t.push(NodeRow::new(
                    NodeKind::KeyValue,
                    Some(parent),
                    span(start, close + 1),
                ));
                let key_end = self.trim_end(start, stop);
                self.string(kv, start, key_end);
                let block = self.t.push(NodeRow::new(
                    NodeKind::Mapping,
                    Some(kv),
                    span(stop, close + 1),
                ));
                self.declarations(block, stop + 1, close)?;
                i = close + 1;
                continue;
            }
            if stop >= to || self.b[stop] != b':' {
                return Err(LanguageError::at("expected `:` after a property", start));
            }
            let name_end = self.trim_end(start, stop);
            let value = self.trivia(stop + 1, to)?;
            let value_end = self.scan(value, to, b";")?;
            let value_end = self.trim_end(value, value_end);
            let kv = self.t.push(NodeRow::new(
                NodeKind::KeyValue,
                Some(parent),
                span(start, value_end.max(stop + 1)),
            ));
            self.string(kv, start, name_end);
            self.string(kv, value, value_end.max(value));
            i = value_end.max(value);
        }
    }
}

/// The members under row `parent`, each a keyvalue's row.
fn members(rows: &[NodeRow], parent: usize) -> impl Iterator<Item = usize> + '_ {
    (0..rows.len()).filter(move |&r| rows[r].parent == Some(parent as u32))
}

/// Whether the mapping at `row` is a block of rules: one whose members'
/// values are mappings. An empty one is not.
fn holds_rules(rows: &[NodeRow], row: usize) -> bool {
    members(rows, row).any(|kv| rows[kv + 2].kind == NodeKind::Mapping)
}

fn print_declarations(rows: &[NodeRow], parent: usize, out: &mut String) {
    let mut first = true;
    for kv in members(rows, parent) {
        if !first {
            out.push_str("; ");
        }
        first = false;
        out.push_str(rows[kv + 1].text.as_deref().unwrap_or(""));
        if rows[kv + 2].kind == NodeKind::Mapping {
            out.push_str(" { ");
            print_declarations(rows, kv + 2, out);
            out.push_str(" }");
        } else {
            out.push_str(": ");
            out.push_str(rows[kv + 2].text.as_deref().unwrap_or(""));
        }
    }
}

fn print_rules(rows: &[NodeRow], parent: usize, indent: &str, out: &mut String) {
    for kv in members(rows, parent) {
        out.push_str(indent);
        out.push_str(rows[kv + 1].text.as_deref().unwrap_or(""));
        let value = kv + 2;
        if rows[value].kind != NodeKind::Mapping {
            // An at-rule statement: `@import url(a.css);`.
            out.push(' ');
            out.push_str(rows[value].text.as_deref().unwrap_or(""));
            out.push_str(";\n");
        } else if holds_rules(rows, value) {
            out.push_str(" {\n");
            print_rules(rows, value, &format!("{indent}  "), out);
            out.push_str(indent);
            out.push_str("}\n");
        } else {
            out.push_str(" { ");
            print_declarations(rows, value, out);
            out.push_str(" }\n");
        }
    }
}

/// One rule of a stylesheet as [`stylesheet`] reads it: where each part
/// is in the text, so a caller that must splice somewhere fig's editor
/// does not insert — before a block's first declaration, between two
/// rules — splices where the parser says, and never scans for itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    /// The selector list, or an at-rule's name and prelude, trimmed.
    pub prelude: Range<usize>,
    /// The whole rule: its prelude through its `}`, or its `;`.
    pub span: Range<usize>,
    pub body: Body,
}

/// What a [`Rule`] holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    /// An at-rule with no block: `@import url(a.css);`.
    Statement,
    /// A declaration block. `open` is the `{`, `close` the `}`; `rules` the
    /// rules nested among the declarations.
    Declarations {
        open: usize,
        close: usize,
        declarations: Vec<Declaration>,
        rules: Vec<Rule>,
    },
    /// An at-rule's block of rules. `inner` is its first rule through its
    /// last, empty at the `}` for a block of none.
    Rules {
        inner: Range<usize>,
        rules: Vec<Rule>,
    },
}

/// One declaration: a property over its value, as written.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub property: String,
    pub value: String,
    /// The property through the end of the value.
    pub span: Range<usize>,
}

impl Rule {
    /// The declarations of a declaration block; none for any other body.
    pub fn declarations(&self) -> &[Declaration] {
        match &self.body {
            Body::Declarations { declarations, .. } => declarations,
            _ => &[],
        }
    }

    /// The value this rule's block gives `property`: the last, where it
    /// repeats, since that is the one that applies.
    pub fn declared(&self, property: &str) -> Option<&str> {
        self.declarations()
            .iter()
            .rev()
            .find(|d| d.property == property)
            .map(|d| d.value.as_str())
    }
}

/// A stylesheet's rules, in order — at-rules' blocks and nested rules
/// held in the rule that holds them. An error is where the text stops
/// being CSS.
pub fn stylesheet(css: &str) -> Result<Vec<Rule>, LanguageError> {
    let table = Css.parse(STYLESHEET, css.as_bytes())?;
    Ok(rules_under(&table.rows, 0))
}

fn range(row: &NodeRow) -> Range<usize> {
    row.span.map_or(0..0, |s| s.start..s.end)
}

fn rules_under(rows: &[NodeRow], parent: usize) -> Vec<Rule> {
    members(rows, parent)
        .filter(|&kv| parent == 0 || rows[kv + 2].kind == NodeKind::Mapping)
        .map(|kv| {
            let value = &rows[kv + 2];
            let body = if value.kind != NodeKind::Mapping {
                Body::Statement
            } else if holds_rule_list(rows[kv + 1].text.as_deref().unwrap_or("")) {
                Body::Rules {
                    inner: range(value),
                    rules: rules_under(rows, kv + 2),
                }
            } else {
                let span = range(value);
                Body::Declarations {
                    open: span.start,
                    close: span.end - 1,
                    declarations: members(rows, kv + 2)
                        .filter(|&d| rows[d + 2].kind != NodeKind::Mapping)
                        .map(|d| Declaration {
                            property: rows[d + 1].text.clone().unwrap_or_default(),
                            value: rows[d + 2].text.clone().unwrap_or_default(),
                            span: range(&rows[d]),
                        })
                        .collect(),
                    rules: rules_under(rows, kv + 2),
                }
            };
            Rule {
                prelude: range(&rows[kv + 1]),
                span: range(&rows[kv]),
                body,
            }
        })
        .collect()
}

/// Whether an at-rule named by `prelude` holds rules rather than
/// declarations: the one test the parser makes when it reads its block.
fn holds_rule_list(prelude: &str) -> bool {
    let name = prelude
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("");
    prelude.starts_with('@') && !DECLARATION_AT_RULES.contains(&name)
}

/// The selectors of a list, trimmed, split at its top-level commas — not
/// at one inside `:is(a, b)` or `[x="a,b"]`.
pub fn selectors(list: &str) -> Vec<&str> {
    let p = Parser {
        src: list,
        b: list.as_bytes(),
        t: NodeTable::new(),
    };
    let mut out = Vec::new();
    let mut at = 0;
    while at <= list.len() {
        let comma = p.scan(at, list.len(), b",").unwrap_or(list.len());
        let s = list[at..comma].trim();
        if !s.is_empty() {
            out.push(s);
        }
        at = comma + 1;
    }
    out
}

/// The value a declaration list (a `style` attribute) gives `property`, as
/// written; the last, where it repeats, since that is the one that applies.
/// `None` when it names no such property, or does not parse.
pub fn declaration(list: &str, property: &str) -> Option<String> {
    let table = Css.parse(DECLARATIONS, list.as_bytes()).ok()?;
    let rows = &table.rows;
    members(rows, 0)
        .filter(|&kv| rows[kv + 2].kind != NodeKind::Mapping)
        .filter(|&kv| rows[kv + 1].text.as_deref() == Some(property))
        .last()
        .and_then(|kv| rows[kv + 2].text.clone())
}

/// `list` with `property` set to `value` — its value replaced where it has
/// one, a declaration added after the last where it has none — or, for
/// `None`, taken out. Every other byte is the list's own.
pub fn with_declaration(list: &str, property: &str, value: Option<&str>) -> Result<String, String> {
    let format = formats()?.declarations;
    let mut editor = fig::Editor::open(list.as_bytes(), format).map_err(|e| e.to_string())?;
    let path = [fig::Segment::Key(property)];
    let result = match value {
        Some(value) => editor.set_value(&path, value),
        None if declaration(list, property).is_none() => return Ok(list.to_string()),
        None => editor.delete_key(&path),
    };
    result.map_err(|e| e.to_string())?;
    editor
        .source()
        .map(str::to_string)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fig::{Editor, Segment};

    fn edit(
        format: Format,
        src: &str,
        op: impl FnOnce(&mut Editor) -> Result<(), fig::Error>,
    ) -> String {
        let mut e = Editor::open(src.as_bytes(), format).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        op(&mut e).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        e.source().unwrap().to_string()
    }

    fn sheet() -> Format {
        formats().expect("css registers").stylesheet
    }

    const INKSCAPE: &str = "fill:none;stroke:#000000;stroke-width:0.36410043px;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1";

    #[test]
    fn a_style_attribute_is_read_as_written() {
        assert_eq!(declaration(INKSCAPE, "stroke").as_deref(), Some("#000000"));
        assert_eq!(
            declaration(INKSCAPE, "stroke-opacity").as_deref(),
            Some("1")
        );
        assert_eq!(declaration(INKSCAPE, "fill-rule"), None);
        assert_eq!(
            declaration(
                "font-family:'Bitstream Vera Sans; Mono';fill:url(#g;1)",
                "fill"
            )
            .as_deref(),
            Some("url(#g;1)")
        );
        assert_eq!(
            declaration("fill: red !important ; fill: blue", "fill").as_deref(),
            Some("blue")
        );
        assert_eq!(
            declaration("/* a */ fill /* b */ : red", "fill").as_deref(),
            Some("red")
        );
    }

    #[test]
    fn a_style_attribute_changes_one_declaration_and_nothing_else() {
        assert_eq!(
            with_declaration(INKSCAPE, "stroke", Some("#c62828")).unwrap(),
            INKSCAPE.replace("stroke:#000000", "stroke:#c62828")
        );
        assert_eq!(
            with_declaration(INKSCAPE, "stroke-opacity", None).unwrap(),
            INKSCAPE.trim_end_matches(";stroke-opacity:1")
        );
        assert_eq!(
            with_declaration(INKSCAPE, "fill", None).unwrap(),
            INKSCAPE.trim_start_matches("fill:none;")
        );
        assert_eq!(
            with_declaration("fill:#000000;stroke:none", "stroke-dasharray", Some("8 6")).unwrap(),
            "fill:#000000;stroke:none; stroke-dasharray: 8 6"
        );
        assert_eq!(
            with_declaration("stop-color:#f7ee5f;", "stop-opacity", Some("1")).unwrap(),
            "stop-color:#f7ee5f; stop-opacity: 1;"
        );
        assert_eq!(
            with_declaration("", "fill", Some("red")).unwrap(),
            "fill: red"
        );
        assert_eq!(with_declaration("fill:red", "fill", None).unwrap(), "");
        assert_eq!(
            with_declaration("fill:red", "stroke", None).unwrap(),
            "fill:red"
        );
    }

    #[test]
    fn an_illustrator_stylesheet_is_edited_in_its_own_spelling() {
        let css = "\n\t.st0{fill-rule:evenodd;clip-rule:evenodd;fill:#B9B9B9;}\n\t.st1{fill-rule:evenodd;clip-rule:evenodd;fill:#E5E5E5;}\n";
        assert_eq!(
            edit(sheet(), css, |e| e.set_value(
                &[Segment::Key(".st1"), Segment::Key("fill")],
                "#c62828"
            )),
            css.replace("fill:#E5E5E5", "fill:#c62828")
        );
        assert_eq!(
            edit(sheet(), css, |e| e.set_value(
                &[Segment::Key(".st0"), Segment::Key("stroke")],
                "#222"
            )),
            css.replace("fill:#B9B9B9;}", "fill:#B9B9B9; stroke: #222;}")
        );
        assert_eq!(
            edit(sheet(), css, |e| e.delete_key(&[
                Segment::Key(".st0"),
                Segment::Key("clip-rule")
            ])),
            css.replacen("clip-rule:evenodd;", "", 1)
        );
        assert_eq!(
            edit(sheet(), css, |e| e.delete_key(&[Segment::Key(".st0")])),
            "\n\t.st1{fill-rule:evenodd;clip-rule:evenodd;fill:#E5E5E5;}\n"
        );
        assert_eq!(
            edit(sheet(), ".a{x:1}.b{y:2}\n", |e| e
                .delete_key(&[Segment::Key(".a")])),
            ".b{y:2}\n"
        );
    }

    #[test]
    fn the_template_stylesheet_parses_and_takes_a_rule() {
        let template = crate::profile::TEMPLATE;
        let open = template
            .find("<style>")
            .expect("the template has a <style>")
            + "<style>".len();
        let close = template.find("</style>").unwrap();
        let css = &template[open..close];
        let table = Css
            .parse(STYLESHEET, css.as_bytes())
            .unwrap_or_else(|e| panic!("{e}"));
        let selectors: Vec<&str> = members(&table.rows, 0)
            .filter_map(|kv| table.rows[kv + 1].text.as_deref())
            .collect();
        assert!(
            selectors.iter().any(|s| s.starts_with("@media")),
            "{selectors:?}"
        );
        // A rule appended at the root is a selector over its block.
        let added = edit(sheet(), css, |e| {
            e.set_value(&[Segment::Key("circle")], "{ fill: none }")
        });
        assert!(added.contains("\n    circle { fill: none }\n"), "{added}");
        assert_eq!(added.replace("    circle { fill: none }\n", ""), css);
    }

    #[test]
    fn a_sheet_opening_with_an_attribute_selector_is_a_block_of_rules() {
        let css = "[data-dash=\"dashed\"] { stroke-dasharray: 8 6 }\nrect { fill: none }\n";
        assert_eq!(
            edit(sheet(), css, |e| e.set_value(
                &[
                    Segment::Key("[data-dash=\"dashed\"]"),
                    Segment::Key("stroke-linecap")
                ],
                "round"
            )),
            css.replace("8 6 }", "8 6; stroke-linecap: round }")
        );
    }

    #[test]
    fn what_is_not_css_is_refused_where_it_goes_wrong() {
        for (src, offset) in [
            ("rect { fill: none", 5),
            ("rect fill: none }", 16),
            ("a { b }", 4),
        ] {
            let err = Css.parse(STYLESHEET, src.as_bytes()).expect_err(src);
            assert_eq!(err.byte_offset, Some(offset), "{src}: {}", err.message);
        }
        assert!(Css.parse(DECLARATIONS, b"fill red").is_err());
    }
}
