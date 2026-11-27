//! The lookup (RFC 0013, "An agent: anchored edits", step 2): where a
//! quote occurs in the Markdown of the version the agent read, with both
//! ends on text, never inside markup.
//!
//! An end falls on text when it sits between two characters of text, or
//! where a block or a whole inline construct, such as `**Friday**`, starts
//! or ends; never between a backslash and the character it escapes, inside
//! a delimiter such as `**`, or inside a link's destination. And the
//! inline constructs open at the quote's start are the ones open at its
//! end, so that a quote such as `on **Fri` that opens a mark without
//! closing it is no match. An occurrence that breaks either is not
//! counted.

use std::collections::BTreeSet;
use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag};

use super::parse::options;

/// The Markdown of a document and the span of each of its top-level
/// blocks in it.
pub(super) struct Parts {
    pub(super) text: String,
    pub(super) spans: Vec<Range<usize>>,
}

impl Parts {
    /// The Markdown of top-level blocks rendered as `parts`, separated by
    /// a blank line, as the rendering joins them.
    pub(super) fn new(parts: Vec<String>) -> Parts {
        let mut text = String::new();
        let mut spans = Vec::with_capacity(parts.len());
        for (n, part) in parts.iter().enumerate() {
            if n > 0 {
                text.push_str("\n\n");
            }
            spans.push(text.len()..text.len() + part.len());
            text.push_str(part);
        }
        Parts { text, spans }
    }

    /// The Markdown of the top-level blocks `first` to `last`.
    pub(super) fn region(&self, first: usize, last: usize) -> Range<usize> {
        self.spans[first].start..self.spans[last].end
    }
}

/// An occurrence of a quote: its range in the Markdown, and the top-level
/// blocks its start and its end fall in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Match {
    pub(super) range: Range<usize>,
    pub(super) first: usize,
    pub(super) last: usize,
}

/// Every occurrence of `quote` in `parts` whose ends fall on text. An
/// empty quote occurs nowhere.
pub(super) fn find(parts: &Parts, quote: &str) -> Vec<Match> {
    let text = &parts.text;
    if quote.is_empty() || quote.len() > text.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (start, _) in text.char_indices() {
        if !text[start..].starts_with(quote) {
            continue;
        }
        let end = start + quote.len();
        let first = parts
            .spans
            .iter()
            .position(|s| s.start <= start && start < s.end);
        let last = parts
            .spans
            .iter()
            .position(|s| s.start < end && end <= s.end);
        let (Some(first), Some(last)) = (first, last) else {
            continue;
        };
        let region = parts.region(first, last);
        let offset = region.start;
        if placed(&text[region], start - offset, end - offset) {
            out.push(Match {
                range: start..end,
                first,
                last,
            });
        }
    }
    out
}

/// Whether a quote from `start` to `end` of `markdown` has both ends on
/// text, with the same inline constructs open at both.
fn placed(markdown: &str, start: usize, end: usize) -> bool {
    let (bounds, inline) = layout(markdown);
    let open = |at: usize| -> Vec<&Range<usize>> {
        inline
            .iter()
            .filter(|r| r.start < at && at < r.end)
            .collect()
    };
    bounds.contains(&start) && bounds.contains(&end) && open(start) == open(end)
}

/// Where an end may fall in `markdown`, and the ranges of its inline
/// constructs: emphasis, strikethrough, links and code spans.
fn layout(markdown: &str) -> (BTreeSet<usize>, Vec<Range<usize>>) {
    let bytes = markdown.as_bytes();
    let mut bounds = BTreeSet::new();
    let mut inline = Vec::new();
    for (event, range) in Parser::new_ext(markdown, options()).into_offset_iter() {
        match event {
            Event::Text(text) => {
                // A text right after a backslash is an escaped character:
                // its start is inside the escape.
                let escaped = range.start > 0 && bytes[range.start - 1] == b'\\';
                let source = &markdown[range.clone()];
                if source == &*text {
                    for (k, _) in source.char_indices() {
                        if k > 0 || !escaped {
                            bounds.insert(range.start + k);
                        }
                    }
                } else if !escaped {
                    bounds.insert(range.start);
                }
                bounds.insert(range.end);
            }
            Event::Code(_) => {
                bounds.insert(range.start);
                bounds.insert(range.end);
                code_bounds(markdown, range.clone(), &mut bounds);
                inline.push(range);
            }
            Event::Start(tag) => {
                bounds.insert(range.start);
                if matches!(
                    tag,
                    Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. }
                ) {
                    bounds.insert(range.end);
                    inline.push(range);
                }
            }
            Event::HardBreak | Event::SoftBreak | Event::Rule => {
                bounds.insert(range.start);
            }
            _ => {}
        }
    }
    (bounds, inline)
}

/// The places between the characters of a code span's text.
fn code_bounds(markdown: &str, range: Range<usize>, bounds: &mut BTreeSet<usize>) {
    let source = &markdown[range.clone()];
    let fence = source.bytes().take_while(|b| *b == b'`').count();
    if source.len() < 2 * fence {
        return;
    }
    let mut inner = range.start + fence..range.end - fence;
    let text = &markdown[inner.clone()];
    if text.len() >= 2 && text.starts_with(' ') && text.ends_with(' ') && !text.trim().is_empty() {
        inner = inner.start + 1..inner.end - 1;
    }
    let text = &markdown[inner.clone()];
    for (k, _) in text.char_indices() {
        bounds.insert(inner.start + k);
    }
    bounds.insert(inner.end);
}
