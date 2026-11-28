//! Tags an edit keeps as they are (RFC 0013, "Parts outside the
//! vocabulary"): the core refuses an edit whose replacement drops a tag
//! its quote contained, and edits one whose replacement keeps every tag of
//! the blocks the quote touches, byte for byte and in the same order.
//!
//! A tag is a whole HTML element of the rendering, from its opening tag to
//! its closing one, with what it holds; tags with nothing between them are
//! one. In the Markdown, each tag reads as one sentinel character, and in
//! the view, each run of parts the view does not show as itself, the
//! parts the rendering writes as tags, reads as the same tag: the `k`th
//! tag of the Markdown is the `k`th run of the view. The diff never writes
//! on a kept tag, so the edit leaves the element, its attributes and what
//! it holds untouched. An edit that drops, changes, adds or moves a tag
//! keeps the tags apart, and is refused.

use std::ops::Range;

use pulldown_cmark::{Event, Parser};

use super::parse::options;
use super::view::{Atom, Content, Old};

/// The character a tag reads as in the Markdown the parser reads back.
pub(super) const SENTINEL: char = '\u{FDD0}';

/// What a kept tag is in the view: a whole block, or `n` atoms of inline
/// content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unit {
    Block,
    Inline(usize),
}

/// The ranges of the tags of `markdown`, in order, or `None` when its HTML
/// is not whole elements, or it holds the sentinel.
pub(super) fn tags(markdown: &str) -> Option<Vec<Range<usize>>> {
    if markdown.contains(SENTINEL) {
        return None;
    }
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut open: Vec<&str> = Vec::new();
    let mut start = 0;
    for (event, range) in Parser::new_ext(markdown, options()).into_offset_iter() {
        let (Event::Html(_) | Event::InlineHtml(_)) = event else {
            continue;
        };
        let source = &markdown[range.clone()];
        let mut at = 0;
        while at < source.len() {
            let rest = &source[at..];
            let skipped = rest.len() - rest.trim_start().len();
            at += skipped;
            if at == source.len() {
                break;
            }
            let (name, closing, len) = tag(&source[at..])?;
            if closing {
                if open.pop()? != name {
                    return None;
                }
            } else {
                if open.is_empty() {
                    start = range.start + at;
                }
                open.push(name);
            }
            at += len;
            if closing && open.is_empty() {
                let end = range.start + at;
                match out.last_mut() {
                    Some(last) if last.end == start => last.end = end,
                    _ => out.push(start..end),
                }
            }
        }
    }
    open.is_empty().then_some(out)
}

/// The tag `source` starts with: its name, whether it closes, and its
/// length. The rendering writes `&` and `"` in an attribute as character
/// references, so a `>` inside quotes is the only one that does not end
/// the tag.
fn tag(source: &str) -> Option<(&str, bool, usize)> {
    let rest = source.strip_prefix('<')?;
    let (closing, rest) = match rest.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let name_len = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':'))
        .unwrap_or(rest.len());
    if name_len == 0 {
        return None;
    }
    let name = &rest[..name_len];
    let mut quoted = false;
    for (k, c) in rest[name_len..].char_indices() {
        match c {
            '"' => quoted = !quoted,
            '>' if !quoted => {
                let len = source.len() - rest.len() + name_len + k + 1;
                return Some((name, closing, len));
            }
            _ => {}
        }
    }
    None
}

/// The Markdown with each tag of `tags` written as the sentinel.
pub(super) fn mask(markdown: &str, tags: &[Range<usize>]) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut at = 0;
    for range in tags {
        out.push_str(&markdown[at..range.start]);
        out.push(SENTINEL);
        at = range.end;
    }
    out.push_str(&markdown[at..]);
    out
}

/// Whether two texts hold the same tags, byte for byte and in order.
pub(super) fn same(a: &str, a_tags: &[Range<usize>], b: &str, b_tags: &[Range<usize>]) -> bool {
    a_tags.len() == b_tags.len()
        && a_tags
            .iter()
            .zip(b_tags)
            .all(|(x, y)| a[x.clone()] == b[y.clone()])
}

/// Marks the parts of `old` the view does not show as itself as kept
/// tags, in order; what each one is.
pub(super) fn keep(old: &mut [Old]) -> Vec<Unit> {
    let mut units = Vec::new();
    for o in old {
        keep_one(o, &mut units);
    }
    units
}

fn keep_one(old: &mut Old, units: &mut Vec<Unit>) {
    match &mut old.block.content {
        Content::Opaque => {
            old.block.content = Content::Kept(units.len());
            units.push(Unit::Block);
        }
        Content::Inline(atoms) => {
            let mut k = 0;
            while k < atoms.len() {
                if !matches!(atoms[k], Atom::Opaque) {
                    k += 1;
                    continue;
                }
                let id = units.len();
                let mut n = 0;
                while matches!(atoms.get(k), Some(Atom::Opaque)) {
                    atoms[k] = Atom::Kept(id);
                    k += 1;
                    n += 1;
                }
                units.push(Unit::Inline(n));
            }
        }
        Content::Blocks(_) => {
            for child in &mut old.children {
                keep_one(child, units);
            }
            old.block.content =
                Content::Blocks(old.children.iter().map(|c| c.block.clone()).collect());
        }
        Content::Leaf | Content::Kept(_) => {}
    }
}
