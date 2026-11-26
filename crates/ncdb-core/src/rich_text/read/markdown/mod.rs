//! The document as Markdown (RFC 0013, "Reading"): CommonMark with
//! GitHub's tables, strikethrough and task lists, in the names TipTap's
//! StarterKit and its task lists use. A node outside that vocabulary, or
//! one whose attributes Markdown has no form for, reads as an HTML tag with
//! its name and attributes around its content, so nothing is hidden from
//! an agent; an attribute the vocabulary does not show, on a node it knows,
//! is kept in the tree and not shown.
//!
//! Blocks are separated by a blank line. A list is tight: its items follow
//! each other on the next line. Two lists in a row take other markers, so
//! that Markdown reads them as two.

mod inline;
mod tag;

use std::collections::BTreeMap;

use ncdb_plugin::Data;

use super::mention;
use super::node::Node;
use inline::Context;

const BLOCK: Context = Context { table: false };
const CELL: Context = Context { table: true };

/// The nodes of a document as Markdown.
pub(crate) fn render(nodes: &[Node]) -> String {
    blocks(nodes)
}

/// Which kind of list a block is, for two lists in a row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    Bullet,
    Ordered,
}

fn list_kind(node: &Node) -> Option<List> {
    match node {
        Node::Element { name, .. } => match name.as_str() {
            "bulletList" | "taskList" => Some(List::Bullet),
            "orderedList" => Some(List::Ordered),
            _ => None,
        },
        Node::Text { .. } => None,
    }
}

/// Blocks, separated by a blank line. Text that stands among blocks reads
/// as a paragraph.
fn blocks(nodes: &[Node]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut previous: Option<(List, bool)> = None;
    let mut i = 0;
    while i < nodes.len() {
        if let Node::Text { .. } = nodes[i] {
            let run = nodes[i..]
                .iter()
                .take_while(|n| matches!(n, Node::Text { .. }))
                .count();
            out.push(inline::render(&nodes[i..i + run], BLOCK));
            previous = None;
            i += run;
            continue;
        }
        let kind = list_kind(&nodes[i]);
        // The second of two lists in a row takes the other marker.
        let other = match (kind, previous) {
            (Some(kind), Some((last, other))) if kind == last => !other,
            _ => false,
        };
        out.push(block(&nodes[i], other));
        previous = kind.map(|kind| (kind, other));
        i += 1;
    }
    out.join("\n\n")
}

/// Whether `node` holds inline content: a paragraph, heading or code
/// block, or any node that holds text.
fn textblock(name: &str, children: &[Node]) -> bool {
    matches!(name, "paragraph" | "heading" | "codeBlock")
        || children.iter().any(|c| matches!(c, Node::Text { .. }))
}

fn block(node: &Node, other_marker: bool) -> String {
    let Node::Element {
        name,
        attrs,
        children,
    } = node
    else {
        return String::new();
    };
    let rendered = match name.as_str() {
        "mention" => mention::of(node).map(|mention| mention.markdown()),
        "paragraph" => Some(inline::render(children, BLOCK)),
        "heading" => heading(attrs, children),
        "blockquote" => Some(quote(children)),
        "bulletList" | "taskList" => Some(list(children, None, other_marker)),
        "orderedList" => start(attrs).map(|start| list(children, Some(start), other_marker)),
        "codeBlock" => Some(code_block(attrs, children)),
        "horizontalRule" if children.is_empty() => Some("---".to_owned()),
        "table" => table(children),
        _ => None,
    };
    rendered.unwrap_or_else(|| tagged(name, attrs, children))
}

/// A node outside the vocabulary: a tag around its inline content, or
/// around its blocks with a blank line on each side, so that Markdown
/// reads them as Markdown.
fn tagged(name: &str, attrs: &BTreeMap<String, Data>, children: &[Node]) -> String {
    let open = tag::open(name, attrs, false);
    let close = tag::close(name);
    if children.is_empty() {
        format!("{open}{close}")
    } else if textblock(name, children) {
        format!("{open}{}{close}", inline::render(children, BLOCK))
    } else {
        format!("{open}\n\n{}\n\n{close}", blocks(children))
    }
}

/// A whole number in `range`, as Yjs writes a JavaScript number.
fn whole(value: &Data, range: std::ops::RangeInclusive<i128>) -> Option<i128> {
    let n = match value {
        Data::Integer(n) => *n,
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => *f as i128,
        _ => return None,
    };
    range.contains(&n).then_some(n)
}

/// A heading, at its level, 1 when it has none. A level Markdown has no
/// form for reads as a tag.
fn heading(attrs: &BTreeMap<String, Data>, children: &[Node]) -> Option<String> {
    let level = match attrs.get("level") {
        None => 1,
        Some(level) => whole(level, 1..=6)?,
    };
    let mut text = inline::render(children, BLOCK);
    // A heading's closing `#`s would be read as markup.
    if text.ends_with('#') && !text.ends_with("\\#") {
        text.insert(text.len() - 1, '\\');
    }
    let marks = "#".repeat(level as usize);
    Some(match text.is_empty() {
        true => marks,
        false => format!("{marks} {text}"),
    })
}

/// Each line of `text` behind `first` on the first line and `rest` on the
/// others; a blank line stays blank.
fn indent(text: &str, first: &str, rest: &str) -> String {
    text.split('\n')
        .enumerate()
        .map(|(n, line)| {
            let prefix = if n == 0 { first } else { rest };
            match line.is_empty() {
                true => prefix.trim_end().to_owned(),
                false => format!("{prefix}{line}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn quote(children: &[Node]) -> String {
    let body = blocks(children);
    body.split('\n')
        .map(|line| match line.is_empty() {
            true => ">".to_owned(),
            false => format!("> {line}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// An ordered list's first number, 1 when it has none.
fn start(attrs: &BTreeMap<String, Data>) -> Option<i128> {
    match attrs.get("start") {
        None => Some(1),
        Some(start) => whole(start, 0..=999_999_999),
    }
}

/// A list: bullets when `start` is `None`, numbers from `start` otherwise.
fn list(items: &[Node], start: Option<i128>, other_marker: bool) -> String {
    items
        .iter()
        .enumerate()
        .map(|(n, item)| {
            let mut marker = match start {
                None if other_marker => "*".to_owned(),
                None => "-".to_owned(),
                Some(start) => {
                    let delimiter = if other_marker { ')' } else { '.' };
                    format!("{}{delimiter}", start + n as i128)
                }
            };
            let continuation = " ".repeat(marker.len() + 1);
            let body = match item {
                Node::Element { name, attrs, children }
                    if name == "listItem" || name == "taskItem" =>
                {
                    if name == "taskItem" {
                        let checked = attrs.get("checked") == Some(&Data::Bool(true));
                        marker.push_str(if checked { " [x]" } else { " [ ]" });
                    }
                    item_body(children)
                }
                other => block(other, false),
            };
            indent(&body, &format!("{marker} "), &continuation)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The blocks of a list item: a list follows on the next line, any other
/// block after a blank line.
fn item_body(children: &[Node]) -> String {
    let mut out = String::new();
    for (n, child) in children.iter().enumerate() {
        if n > 0 {
            out.push_str(match list_kind(child) {
                Some(_) => "\n",
                None => "\n\n",
            });
        }
        out.push_str(&blocks(std::slice::from_ref(child)));
    }
    out
}

/// The text of `nodes` with no markup, for a code block.
fn raw(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text { text, .. } => out.push_str(text),
            Node::Element { children, .. } => raw(children, out),
        }
    }
}

/// A fenced code block with its language; the fence is longer than any
/// run of its character in the code.
fn code_block(attrs: &BTreeMap<String, Data>, children: &[Node]) -> String {
    let mut code = String::new();
    raw(children, &mut code);
    let language = match attrs.get("language") {
        Some(Data::Text(language)) => language.replace('\n', " "),
        _ => String::new(),
    };
    let fence = if language.contains('`') { '~' } else { '`' };
    let mut longest = 0;
    let mut run = 0;
    for c in code.chars() {
        run = if c == fence { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = fence.to_string().repeat((longest + 1).max(3));
    match code.is_empty() {
        true => format!("{fence}{language}\n{fence}"),
        false => format!("{fence}{language}\n{code}\n{fence}"),
    }
}

/// A table, its first row the header. A table without rows of cells reads
/// as a tag.
fn table(rows: &[Node]) -> Option<String> {
    let mut cells: Vec<Vec<String>> = Vec::new();
    for row in rows {
        let Node::Element { name, children, .. } = row else {
            return None;
        };
        if name != "tableRow" {
            return None;
        }
        cells.push(children.iter().map(cell).collect());
    }
    let columns = cells.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return None;
    }
    let line = |row: &[String]| {
        let mut row = row.to_vec();
        row.resize(columns, String::new());
        format!("| {} |", row.join(" | "))
    };
    let mut out = vec![line(&cells[0]), line(&vec!["---".to_owned(); columns])];
    out.extend(cells[1..].iter().map(|row| line(row)));
    Some(out.join("\n"))
}

/// A table cell on one line: its paragraphs joined by `<br>`.
fn cell(node: &Node) -> String {
    let (name, children) = match node {
        Node::Element { name, children, .. } => (name, children),
        Node::Text { .. } => return inline::render(std::slice::from_ref(node), CELL),
    };
    if name != "tableCell" && name != "tableHeader" || textblock(name, children) {
        return match textblock(name, children) {
            true => inline::render(children, CELL),
            false => block(node, false).replace('|', "\\|").replace('\n', "<br>"),
        };
    }
    children
        .iter()
        .map(|child| match child {
            Node::Element { name, children, .. } if name == "paragraph" => {
                inline::render(children, CELL)
            }
            other => block(other, false).replace('|', "\\|").replace('\n', "<br>"),
        })
        .collect::<Vec<_>>()
        .join("<br>")
}
