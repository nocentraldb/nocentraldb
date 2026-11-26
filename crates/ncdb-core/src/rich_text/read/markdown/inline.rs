//! The inline content of a block as Markdown: runs of text with their
//! marks, hard breaks, and inline nodes outside the vocabulary as tags.
//!
//! Marks nest: of the marks that open together, the one that runs
//! furthest opens first, so a mark seldom has to close and open again; a
//! link is outside every other mark, code inside every other. Whitespace
//! at the edge of a mark moves outside it, since CommonMark does not let
//! emphasis start or end next to a space. Text that Markdown would read as
//! markup is escaped.

use std::collections::BTreeMap;

use ncdb_plugin::Data;

use super::super::mention;
use super::super::node::{Mark, Node};
use super::tag;

/// Where the inline content stands: in a table cell, a line break is
/// `<br>` and `|` is escaped.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Context {
    pub(super) table: bool,
}

/// A mark as Markdown writes it, in nesting order: links outermost, then
/// tags, then bold, italic and strikethrough.
#[derive(Clone, Debug, PartialEq)]
enum Form {
    Link(String),
    Tag(String, BTreeMap<String, Data>),
    Bold,
    Italic,
    Strike,
}

/// What one piece of inline content is.
enum Leaf<'a> {
    Text(String),
    Code(String),
    Node(&'a Node),
}

struct Segment<'a> {
    forms: Vec<Form>,
    leaf: Leaf<'a>,
}

/// The Markdown form of a mark: `None` for code, which is a leaf of its
/// own, and for underline and highlight, which Markdown has no form for
/// and which read as their text.
fn form(mark: &Mark) -> Option<Form> {
    Some(match mark.name.as_str() {
        "bold" => Form::Bold,
        "italic" => Form::Italic,
        "strike" => Form::Strike,
        "code" | "underline" | "highlight" => return None,
        "link" => match mark.attrs.get("href") {
            Some(Data::Text(href)) => Form::Link(href.clone()),
            _ => Form::Tag(mark.name.clone(), mark.attrs.clone()),
        },
        _ => Form::Tag(mark.name.clone(), mark.attrs.clone()),
    })
}

/// Where a form nests: lower is further out.
fn rank(form: &Form) -> (u8, String) {
    match form {
        Form::Link(_) => (0, String::new()),
        Form::Tag(name, _) => (1, name.clone()),
        Form::Bold => (2, String::new()),
        Form::Italic => (3, String::new()),
        Form::Strike => (4, String::new()),
    }
}

fn segments(nodes: &[Node]) -> Vec<Segment<'_>> {
    let mut out: Vec<Segment<'_>> = Vec::new();
    for node in nodes {
        let segment = match node {
            Node::Text { text, marks } => {
                let mut forms: Vec<Form> = marks.iter().filter_map(form).collect();
                // Stable: tags of one name keep the marks' order.
                forms.sort_by_key(rank);
                forms.dedup();
                let code = marks.iter().any(|m| m.name == "code");
                Segment {
                    forms,
                    leaf: match code {
                        true => Leaf::Code(text.clone()),
                        false => Leaf::Text(text.clone()),
                    },
                }
            }
            Node::Element { .. } => Segment {
                forms: Vec::new(),
                leaf: Leaf::Node(node),
            },
        };
        // Runs that read alike join, so that a code span is one span.
        if let Some(last) = out.last_mut()
            && last.forms == segment.forms
        {
            match (&mut last.leaf, &segment.leaf) {
                (Leaf::Text(a), Leaf::Text(b)) | (Leaf::Code(a), Leaf::Code(b)) => {
                    a.push_str(b);
                    continue;
                }
                _ => {}
            }
        }
        out.push(segment);
    }
    // Whitespace alone opens no mark.
    for k in 0..out.len() {
        if let Leaf::Text(text) = &out[k].leaf
            && text.chars().all(char::is_whitespace)
        {
            let before = k.checked_sub(1).map(|j| out[j].forms.clone()).unwrap_or_default();
            out[k].forms.retain(|f| before.contains(f));
        }
    }
    out
}

/// The inline content `nodes` as Markdown.
pub(super) fn render(nodes: &[Node], context: Context) -> String {
    let segments = segments(nodes);
    let mut out = String::new();
    let mut stack: Vec<Form> = Vec::new();
    let mut held = String::new();
    for (k, segment) in segments.iter().enumerate() {
        let keep = stack
            .iter()
            .position(|f| !segment.forms.contains(f))
            .unwrap_or(stack.len());
        for form in stack.drain(keep..).rev() {
            out.push_str(&close(&form, context));
        }
        out.push_str(&std::mem::take(&mut held));
        let mut opening: Vec<&Form> = segment.forms.iter().filter(|f| !stack.contains(f)).collect();
        opening.sort_by_key(|f| std::cmp::Reverse(end(&segments, k, f)));
        let mut text = match &segment.leaf {
            Leaf::Text(text) => text.as_str(),
            _ => "",
        };
        if !opening.is_empty() {
            let trimmed = text.trim_start();
            out.push_str(&text[..text.len() - trimmed.len()]);
            text = trimmed;
        }
        for form in opening {
            out.push_str(&open(form));
            stack.push(form.clone());
        }
        let closes = match segments.get(k + 1) {
            Some(next) => stack.iter().any(|f| !next.forms.contains(f)),
            None => !stack.is_empty(),
        };
        match &segment.leaf {
            Leaf::Text(_) => {
                let trimmed = match closes {
                    true => text.trim_end(),
                    false => text,
                };
                escape(&mut out, trimmed, context);
                held = text[trimmed.len()..].to_owned();
            }
            Leaf::Code(code) => out.push_str(&code_span(code, context)),
            Leaf::Node(node) => node_inline(&mut out, node, context),
        }
    }
    for form in stack.drain(..).rev() {
        out.push_str(&close(&form, context));
    }
    out.push_str(&held);
    out
}

/// The index of the first segment after `k` that `form` does not cover.
fn end(segments: &[Segment<'_>], k: usize, form: &Form) -> usize {
    (k + 1..segments.len())
        .find(|&i| !segments[i].forms.contains(form))
        .unwrap_or(segments.len())
}

fn open(form: &Form) -> String {
    match form {
        Form::Link(_) => "[".to_owned(),
        Form::Tag(name, attrs) => tag::open(name, attrs, false),
        Form::Bold => "**".to_owned(),
        Form::Italic => "*".to_owned(),
        Form::Strike => "~~".to_owned(),
    }
}

fn close(form: &Form, context: Context) -> String {
    match form {
        Form::Link(href) => format!("]({})", destination(href, context)),
        Form::Tag(name, _) => tag::close(name),
        Form::Bold => "**".to_owned(),
        Form::Italic => "*".to_owned(),
        Form::Strike => "~~".to_owned(),
    }
}

/// A link destination: in angle brackets when it holds a space or a
/// control character, otherwise with parentheses escaped.
fn destination(href: &str, context: Context) -> String {
    let angle = href.chars().any(|c| c == ' ' || c.is_control());
    let mut out = String::new();
    if angle {
        out.push('<');
    }
    for c in href.chars() {
        let escaped = match c {
            '<' | '>' | '\\' => true,
            '(' | ')' => !angle,
            '|' => context.table,
            _ => false,
        };
        if escaped {
            out.push('\\');
        }
        out.push(c);
    }
    if angle {
        out.push('>');
    }
    out
}

/// A code span, its fence one backtick longer than the longest run of
/// backticks it holds.
fn code_span(code: &str, context: Context) -> String {
    let mut code = code.replace('\n', " ");
    if context.table {
        code = code.replace('|', "\\|");
    }
    let mut longest = 0;
    let mut run = 0;
    for c in code.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest + 1);
    let pad = code.starts_with('`')
        || code.ends_with('`')
        || (code.starts_with(' ') && code.ends_with(' ') && !code.trim().is_empty());
    match pad {
        true => format!("{fence} {code} {fence}"),
        false => format!("{fence}{code}{fence}"),
    }
}

fn node_inline(out: &mut String, node: &Node, context: Context) {
    let Node::Element {
        name,
        attrs,
        children,
    } = node
    else {
        return;
    };
    if let Some(mention) = mention::of(node) {
        out.push_str(&mention.markdown());
        return;
    }
    match name.as_str() {
        "hardBreak" => hard_break(out, context),
        _ => {
            out.push_str(&tag::open(name, attrs, context.table));
            out.push_str(&render(children, context));
            out.push_str(&tag::close(name));
        }
    }
}

fn hard_break(out: &mut String, context: Context) {
    match context.table {
        true => out.push_str("<br>"),
        false => out.push_str("\\\n"),
    }
}

/// Whether what `out` holds of its last line is blank, so that text
/// written next starts a line for Markdown.
fn at_line_start(out: &str) -> bool {
    out.rsplit('\n')
        .next()
        .is_none_or(|line| line.chars().all(|c| c == ' '))
}

/// Writes `text` escaped: the characters that open inline markup always,
/// `&` where it would start a character reference, and at the start of a
/// line the characters that would start a block.
fn escape(out: &mut String, text: &str, context: Context) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            hard_break(out, context);
            i += 1;
            continue;
        }
        if c != ' ' && at_line_start(out) {
            match c {
                '#' | '>' | '+' | '-' | '=' => out.push('\\'),
                '0'..='9' => {
                    let digits = chars[i..].iter().take_while(|c| c.is_ascii_digit()).count();
                    if digits <= 9 && matches!(chars.get(i + digits), Some('.' | ')')) {
                        out.extend(&chars[i..i + digits]);
                        out.push('\\');
                        out.push(chars[i + digits]);
                        i += digits + 1;
                        continue;
                    }
                }
                _ => {}
            }
        }
        match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '~' => out.push('\\'),
            '|' if context.table => out.push('\\'),
            '&' if reference(&chars[i + 1..]) => out.push('\\'),
            _ => {}
        }
        out.push(c);
        i += 1;
    }
}

/// Whether the text after a `&` would make it a character reference.
fn reference(after: &[char]) -> bool {
    let name = after
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == '#')
        .count();
    name > 0 && after.get(name) == Some(&';')
}
