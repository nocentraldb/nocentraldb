//! The document as plain text (RFC 0013, "Reading"), for previews: the text
//! of each block that holds text on a line of its own, without markup.
//! Blocks inside lists, quotes, tables and nodes outside the vocabulary
//! give a line each; a hard break starts a new line, a code block keeps
//! its lines, a rule gives none, and a mention, which holds no name, reads
//! as `@`.

use super::mention;
use super::node::Node;

/// The nodes of a document as plain text.
pub(crate) fn render(nodes: &[Node]) -> String {
    let mut lines = Vec::new();
    blocks(nodes, &mut lines);
    lines.join("\n")
}

fn blocks(nodes: &[Node], lines: &mut Vec<String>) {
    let mut i = 0;
    while i < nodes.len() {
        match &nodes[i] {
            Node::Text { .. } => {
                let run = nodes[i..]
                    .iter()
                    .take_while(|n| matches!(n, Node::Text { .. }))
                    .count();
                lines.push(inline(&nodes[i..i + run]));
                i += run;
                continue;
            }
            Node::Element { name, children, .. } => {
                let holds_text = matches!(name.as_str(), "paragraph" | "heading" | "codeBlock")
                    || children.iter().any(|c| matches!(c, Node::Text { .. }));
                if holds_text {
                    lines.push(inline(children));
                } else {
                    blocks(children, lines);
                }
            }
        }
        i += 1;
    }
}

fn inline(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Text { text, .. } => out.push_str(text),
            Node::Element { name, .. } if name == "hardBreak" => out.push('\n'),
            Node::Element { .. } if mention::of(node).is_some() => out.push('@'),
            Node::Element { children, .. } => out.push_str(&inline(children)),
        }
    }
    out
}
