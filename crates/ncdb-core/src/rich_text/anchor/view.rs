//! What the Markdown of a document shows, as blocks and inline atoms: the
//! form in which the core compares the blocks a quote touches with the
//! blocks its replacement reads as. A block read from Yrs keeps the handle
//! of its element, and each atom where it sits, so that the diff writes on
//! exactly those items.
//!
//! The view shows what the Markdown rendering shows (RFC 0013, "Reading"):
//! TipTap's standard nodes with the attributes Markdown has a form for, and
//! the marks bold, italic, strike, code and link. Underline and highlight,
//! which Markdown reads as their text, are no part of it, and neither is an
//! attribute the rendering does not show: the diff never touches them. A
//! node or mark outside the vocabulary, which the rendering shows as an
//! HTML tag, reads as opaque: the core does not edit it from Markdown yet,
//! so a quote whose blocks hold one is refused.

use std::collections::{BTreeMap, BTreeSet};

use yrs::types::Attrs;
use yrs::types::text::YChange;
use yrs::types::xml::{Xml, XmlFragment, XmlOut};
use yrs::{Any, Number, Out, ReadTxn, Text, XmlElementRef, XmlTextRef};

use super::super::read::mark_name;

/// A mark the Markdown shows.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Mark {
    Bold,
    Italic,
    Strike,
    Code,
    Link(String),
}

impl Mark {
    /// The mark's name in the document.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Mark::Bold => "bold",
            Mark::Italic => "italic",
            Mark::Strike => "strike",
            Mark::Code => "code",
            Mark::Link(_) => "link",
        }
    }
}

/// The marks of a character.
pub(super) type Marks = BTreeSet<Mark>;

/// One unit of inline content.
#[derive(Clone, Debug)]
pub(super) enum Atom {
    /// A character and its marks.
    Char(char, Marks),
    /// An inline node: a hard break or a mention.
    Node(Block),
    /// Something the view does not show as itself; equal to nothing.
    Opaque,
}

/// What a block holds.
#[derive(Clone, Debug)]
pub(super) enum Content {
    /// Blocks, as a list or a quote holds them.
    Blocks(Vec<Block>),
    /// Inline content, as a paragraph, heading or code block holds it.
    Inline(Vec<Atom>),
    /// Nothing, as a rule, a hard break or a mention.
    Leaf,
    /// Something the view does not show as itself; equal to nothing.
    Opaque,
}

/// A block or an inline node, with the attributes Markdown shows, each as
/// the text of its canonical value.
#[derive(Clone, Debug)]
pub(super) struct Block {
    pub(super) name: String,
    pub(super) attrs: BTreeMap<&'static str, String>,
    pub(super) content: Content,
}

/// The nodes whose content is inline.
const TEXTBLOCKS: [&str; 3] = ["paragraph", "heading", "codeBlock"];
/// The nodes whose content is blocks.
const CONTAINERS: [&str; 10] = [
    "blockquote",
    "bulletList",
    "orderedList",
    "taskList",
    "listItem",
    "taskItem",
    "table",
    "tableRow",
    "tableHeader",
    "tableCell",
];

impl Block {
    /// Whether the Markdown loses this block: an empty paragraph among
    /// blocks reads as nothing.
    pub(super) fn invisible(&self) -> bool {
        self.name == "paragraph"
            && matches!(&self.content, Content::Inline(atoms) if atoms.is_empty())
    }

    /// Whether `self` and `other` are the same kind of block, which the
    /// diff edits in place: the same name, and content of the same kind.
    pub(super) fn same_kind(&self, other: &Block) -> bool {
        self.name == other.name
            && matches!(
                (&self.content, &other.content),
                (Content::Blocks(_), Content::Blocks(_))
                    | (Content::Inline(_), Content::Inline(_))
                    | (Content::Leaf, Content::Leaf)
            )
    }
}

/// Whether a block holds blocks whose empty paragraphs the Markdown loses:
/// all but a table cell, whose paragraphs read on one line.
pub(super) fn drops_empty(name: &str) -> bool {
    !matches!(name, "tableHeader" | "tableCell")
}

/// Equal as the Markdown shows them. Whitespace takes no emphasis and no
/// link in Markdown, which moves it outside the mark, so only its code
/// mark counts.
pub(super) fn atom_eq(a: &Atom, b: &Atom) -> bool {
    match (a, b) {
        (Atom::Char(x, m), Atom::Char(y, n)) => x == y && shown(*x, m) == shown(*y, n),
        (Atom::Node(x), Atom::Node(y)) => block_eq(x, y),
        _ => false,
    }
}

/// The marks of `c` that the Markdown shows.
pub(super) fn shown(c: char, marks: &Marks) -> Marks {
    match c.is_whitespace() {
        true => marks
            .iter()
            .filter(|m| **m == Mark::Code)
            .cloned()
            .collect(),
        false => marks.clone(),
    }
}

pub(super) fn block_eq(a: &Block, b: &Block) -> bool {
    a.name == b.name
        && a.attrs == b.attrs
        && match (&a.content, &b.content) {
            (Content::Blocks(x), Content::Blocks(y)) => blocks_eq(x, y),
            (Content::Inline(x), Content::Inline(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(a, b)| atom_eq(a, b))
            }
            (Content::Leaf, Content::Leaf) => true,
            _ => false,
        }
}

pub(super) fn blocks_eq(a: &[Block], b: &[Block]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| block_eq(a, b))
}

/// A block read from the document, with the handles the diff writes on.
#[derive(Clone)]
pub(super) struct Old {
    /// What the Markdown shows of it.
    pub(super) block: Block,
    /// Its element.
    pub(super) element: XmlElementRef,
    /// Its index among its parent's children.
    pub(super) index: u32,
    /// Its blocks, the ones the Markdown loses left out.
    pub(super) children: Vec<Old>,
    /// Where each of its atoms sits, when its content is inline.
    pub(super) atoms: Vec<At>,
}

/// Where an atom sits in the document.
#[derive(Clone)]
pub(super) enum At {
    /// A character: its text, its child index in the block, its offset in
    /// UTF-16 units, its length, and its formatting attributes as Yrs
    /// holds them.
    Char {
        text: XmlTextRef,
        child: u32,
        offset: u32,
        len: u32,
        raw: Attrs,
    },
    /// An inline element and its child index in the block.
    Node { child: u32 },
    /// Something the view does not show.
    Opaque { child: u32 },
}

/// The blocks of the document's root, or `None` when the root holds
/// something other than elements, which TipTap never writes there.
pub(super) fn root<T: ReadTxn>(txn: &T, root: &impl XmlFragment) -> Option<Vec<Old>> {
    let mut out = Vec::new();
    for (index, child) in root.children(txn).enumerate() {
        let XmlOut::Element(element) = child else {
            return None;
        };
        out.push(element_of(txn, element, index as u32));
    }
    Some(out)
}

fn element_of<T: ReadTxn>(txn: &T, element: XmlElementRef, index: u32) -> Old {
    let name = element.tag().to_string();
    let mut old = Old {
        block: Block {
            name: name.clone(),
            attrs: BTreeMap::new(),
            content: Content::Opaque,
        },
        element: element.clone(),
        index,
        children: Vec::new(),
        atoms: Vec::new(),
    };
    let Some(attrs) = attrs(txn, &element, &name) else {
        return old;
    };
    old.block.attrs = attrs;
    if TEXTBLOCKS.contains(&name.as_str()) {
        let code = name == "codeBlock";
        let (atoms, at) = inline(txn, &element, code);
        old.block.content = Content::Inline(atoms);
        old.atoms = at;
    } else if CONTAINERS.contains(&name.as_str()) {
        let mut children = Vec::new();
        for (index, child) in element.children(txn).enumerate() {
            let XmlOut::Element(child) = child else {
                return old;
            };
            let child = element_of(txn, child, index as u32);
            if !(drops_empty(&name) && child.block.invisible()) {
                children.push(child);
            }
        }
        old.block.content = Content::Blocks(children.iter().map(|c| c.block.clone()).collect());
        old.children = children;
    } else if matches!(name.as_str(), "horizontalRule" | "hardBreak" | "mention")
        && element.len(txn) == 0
    {
        old.block.content = Content::Leaf;
    }
    old
}

/// The attributes Markdown shows of `element`, by its name, or `None` when
/// one has a value Markdown has no form for, which reads as a tag.
fn attrs<T: ReadTxn>(
    txn: &T,
    element: &XmlElementRef,
    name: &str,
) -> Option<BTreeMap<&'static str, String>> {
    let get = |key: &str| match element.get_attribute(txn, key) {
        Some(Out::Any(any)) => Some(any),
        Some(_) => Some(Any::Undefined),
        None => None,
    };
    let mut out = BTreeMap::new();
    match name {
        "heading" => {
            let level = match get("level") {
                None => 1,
                Some(level) => whole(&level, 1..=6)?,
            };
            out.insert("level", level.to_string());
        }
        "orderedList" => {
            let start = match get("start") {
                None => 1,
                Some(start) => whole(&start, 0..=999_999_999)?,
            };
            out.insert("start", start.to_string());
        }
        "taskItem" => {
            let checked = matches!(get("checked"), Some(Any::Bool(true)));
            out.insert("checked", checked.to_string());
        }
        "codeBlock" => {
            if let Some(Any::String(language)) = get("language") {
                if language.contains('\n') {
                    return None;
                }
                out.insert("language", language.to_string());
            }
        }
        "mention" => {
            for key in ["space", "id"] {
                match get(key) {
                    Some(Any::String(id)) if is_id(&id) => out.insert(key, id.to_string()),
                    _ => return None,
                };
            }
        }
        _ => {}
    }
    Some(out)
}

/// Whether `text` is an id as RFC 0009 writes it: 32 lowercase
/// hexadecimal digits.
pub(super) fn is_id(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A whole number in `range`, as Yjs writes a JavaScript number.
fn whole(value: &Any, range: std::ops::RangeInclusive<i64>) -> Option<i64> {
    let n = match value {
        Any::Number(Number::Int(n)) => *n,
        Any::Number(Number::Float(f)) if f.fract() == 0.0 && f.abs() < 1e15 => *f as i64,
        _ => return None,
    };
    range.contains(&n).then_some(n)
}

/// The inline content of a text block and where each atom sits.
fn inline<T: ReadTxn>(txn: &T, element: &XmlElementRef, code: bool) -> (Vec<Atom>, Vec<At>) {
    let (mut atoms, mut at) = (Vec::new(), Vec::new());
    for (child, node) in element.children(txn).enumerate() {
        let child = child as u32;
        match node {
            XmlOut::Text(text) => {
                let mut offset = 0;
                for chunk in text.diff(txn, YChange::identity) {
                    let Out::Any(Any::String(run)) = chunk.insert else {
                        atoms.push(Atom::Opaque);
                        at.push(At::Opaque { child });
                        offset += 1;
                        continue;
                    };
                    let raw: Attrs = chunk.attributes.map(|a| *a).unwrap_or_default();
                    let marks = match code {
                        true => Some(Marks::new()),
                        false => marks(&raw),
                    };
                    for c in run.chars() {
                        let len = c.len_utf16() as u32;
                        atoms.push(match &marks {
                            Some(marks) => Atom::Char(c, marks.clone()),
                            None => Atom::Opaque,
                        });
                        at.push(At::Char {
                            text: text.clone(),
                            child,
                            offset,
                            len,
                            raw: raw.clone(),
                        });
                        offset += len;
                    }
                }
            }
            XmlOut::Element(inner) => {
                let old = element_of(txn, inner, child);
                let leaf = matches!(old.block.content, Content::Leaf)
                    && matches!(old.block.name.as_str(), "hardBreak" | "mention");
                match leaf && !code {
                    true => {
                        atoms.push(Atom::Node(old.block));
                        at.push(At::Node { child });
                    }
                    false => {
                        atoms.push(Atom::Opaque);
                        at.push(At::Opaque { child });
                    }
                }
            }
            XmlOut::Fragment(_) => {
                atoms.push(Atom::Opaque);
                at.push(At::Opaque { child });
            }
        }
    }
    (atoms, at)
}

/// The marks Markdown shows of a run's formatting attributes, or `None`
/// when it holds a mark the rendering shows as a tag. Underline and
/// highlight read as their text and are no part of the view.
fn marks(raw: &Attrs) -> Option<Marks> {
    let mut out = Marks::new();
    for (key, value) in raw {
        let attrs = match value {
            Any::Null | Any::Undefined | Any::Bool(false) => continue,
            Any::Map(map) => Some(map),
            _ => None,
        };
        match mark_name(key) {
            "bold" => out.insert(Mark::Bold),
            "italic" => out.insert(Mark::Italic),
            "strike" => out.insert(Mark::Strike),
            "code" => out.insert(Mark::Code),
            "underline" | "highlight" => continue,
            "link" => match attrs.and_then(|a| a.get("href")) {
                Some(Any::String(href)) => out.insert(Mark::Link(href.to_string())),
                _ => return None,
            },
            _ => return None,
        };
    }
    Some(out)
}
