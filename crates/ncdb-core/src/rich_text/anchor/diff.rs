//! The smallest change (RFC 0013, "An agent: anchored edits", step 3),
//! written directly on the Yrs document: blocks the replacement leaves
//! alone are not touched; a block whose kind stays is edited in place,
//! its attributes set one by one and its text by characters, its marks by
//! ranges; only a block whose type the replacement changes is deleted and
//! written anew, since Yjs cannot rename an element (F14).
//!
//! Text is compared by its common start and end: the characters between
//! go, the replacement's characters come, and kept characters whose marks
//! differ are formatted. New characters take the marks the replacement
//! gives them, and from the text they are written next to the marks
//! Markdown does not show, such as underline, and a link's attributes
//! other than its address.

use std::collections::HashMap;
use std::sync::Arc;

use yrs::types::Attrs;
use yrs::types::text::YChange;
use yrs::types::xml::{Xml, XmlElementPrelim, XmlFragment, XmlOut, XmlTextPrelim};
use yrs::{Any, Out, Text, TransactionMut, XmlElementRef, XmlTextRef};

use super::super::read::mark_name;
use super::view::{At, Atom, Block, Content, Mark, Marks, Old, block_eq, shown};

/// The core does not make this change.
#[derive(Debug)]
pub(super) struct Refused;

/// Turns the blocks `old`, children of `parent`, into `new`. An empty
/// `old` gets `new` at `start`.
pub(super) fn blocks(
    txn: &mut TransactionMut,
    parent: &impl XmlFragment,
    old: &[Old],
    new: &[Block],
    start: u32,
) -> Result<(), Refused> {
    let n = old.len().min(new.len());
    let mut pre = 0;
    while pre < n && block_eq(&old[pre].block, &new[pre]) {
        pre += 1;
    }
    let mut suf = 0;
    while suf < n - pre && block_eq(&old[old.len() - 1 - suf].block, &new[new.len() - 1 - suf]) {
        suf += 1;
    }
    let (om, nm) = (&old[pre..old.len() - suf], &new[pre..new.len() - suf]);
    let m = om.len().min(nm.len());
    let mut front = 0;
    while front < m && om[front].block.same_kind(&nm[front]) {
        front += 1;
    }
    let mut back = 0;
    while back < m - front
        && om[om.len() - 1 - back]
            .block
            .same_kind(&nm[nm.len() - 1 - back])
    {
        back += 1;
    }
    for k in 0..back {
        pair(txn, &om[om.len() - 1 - k], &nm[nm.len() - 1 - k])?;
    }
    let gone = &om[front..om.len() - back];
    if gone.iter().any(|o| !deletable(&o.block)) {
        return Err(Refused);
    }
    for o in gone.iter().rev() {
        parent.remove_range(txn, o.index, 1);
    }
    let at = match (pre + front).checked_sub(1) {
        Some(k) => old[k].index + 1,
        None => old.first().map_or(start, |o| o.index),
    };
    write_blocks(txn, parent, at, &nm[front..nm.len() - back])?;
    for k in (0..front).rev() {
        pair(txn, &om[k], &nm[k])?;
    }
    Ok(())
}

/// Whether the diff may delete `block`: never a part the view does not
/// show, so an edit never drops a tag.
fn deletable(block: &Block) -> bool {
    match &block.content {
        Content::Opaque => false,
        Content::Leaf => true,
        Content::Blocks(blocks) => blocks.iter().all(deletable),
        Content::Inline(atoms) => atoms.iter().all(|a| !matches!(a, Atom::Opaque)),
    }
}

/// Edits `old` in place into `new`, a block of the same kind.
fn pair(txn: &mut TransactionMut, old: &Old, new: &Block) -> Result<(), Refused> {
    for (key, value) in &new.attrs {
        if old.block.attrs.get(key) != Some(value) {
            old.element.insert_attribute(txn, *key, any(key, value)?);
        }
    }
    for key in old.block.attrs.keys() {
        if !new.attrs.contains_key(key) {
            old.element.remove_attribute(txn, key);
        }
    }
    match &new.content {
        Content::Blocks(children) => blocks(txn, &old.element, &old.children, children, 0),
        Content::Inline(atoms) => inline(txn, old, atoms),
        Content::Leaf => Ok(()),
        Content::Opaque => Err(Refused),
    }
}

/// An attribute's value as TipTap writes it.
fn any(key: &str, value: &str) -> Result<Any, Refused> {
    Ok(match key {
        "level" | "start" => Any::from(value.parse::<i64>().map_err(|_| Refused)?),
        "checked" => Any::Bool(value == "true"),
        _ => Any::from(value),
    })
}

/// Whether two atoms are the same text or node, whatever their marks.
fn same(a: &Atom, b: &Atom) -> bool {
    match (a, b) {
        (Atom::Char(x, _), Atom::Char(y, _)) => x == y,
        (Atom::Node(x), Atom::Node(y)) => block_eq(x, y),
        _ => false,
    }
}

/// Turns the inline content of `old` into `new`.
fn inline(txn: &mut TransactionMut, old: &Old, new: &[Atom]) -> Result<(), Refused> {
    let Content::Inline(atoms) = &old.block.content else {
        return Err(Refused);
    };
    let n = atoms.len().min(new.len());
    let mut pre = 0;
    while pre < n && same(&atoms[pre], &new[pre]) {
        pre += 1;
    }
    let mut suf = 0;
    while suf < n - pre && same(&atoms[atoms.len() - 1 - suf], &new[new.len() - 1 - suf]) {
        suf += 1;
    }
    let middle = pre..atoms.len() - suf;
    if middle.clone().any(|k| matches!(atoms[k], Atom::Opaque)) {
        return Err(Refused);
    }
    // Marks of kept characters, before any text moves.
    let pairs: Vec<(usize, usize)> = (0..pre)
        .chain(atoms.len() - suf..atoms.len())
        .map(|k| {
            (
                k,
                if k < pre {
                    k
                } else {
                    k + new.len() - atoms.len()
                },
            )
        })
        .collect();
    format(txn, &old.atoms, atoms, new, &pairs)?;
    // The replacement's characters and nodes come right after the kept
    // start, before the characters they replace go, so that text a person
    // typed next to the replaced characters meanwhile stays after the new
    // ones.
    let source = pre
        .checked_sub(1)
        .and_then(|k| raw(&old.atoms[k]))
        .or_else(|| middle.clone().find_map(|k| raw(&old.atoms[k])));
    let cursor = match pre.checked_sub(1).map(|k| &old.atoms[k]) {
        Some(At::Char {
            text,
            child,
            offset,
            len,
            ..
        }) => Cursor::Text(text.clone(), *child, offset + len),
        Some(At::Node { child } | At::Opaque { child }) => Cursor::Before(child + 1),
        None => Cursor::Before(0),
    };
    let moved = insert(
        txn,
        &old.element,
        cursor,
        &new[pre..new.len() - suf],
        source,
    )?;
    // The characters and nodes between go, from the end.
    for k in middle.rev() {
        match &old.atoms[k] {
            At::Char {
                text, offset, len, ..
            } => {
                let shift = match &moved.into {
                    Some((into, inserted)) if into == text => *inserted,
                    _ => 0,
                };
                text.remove_range(txn, offset + shift, *len);
            }
            At::Node { child } => old.element.remove_range(txn, child + moved.children, 1),
            At::Opaque { .. } => return Err(Refused),
        }
    }
    Ok(())
}

/// The formatting attributes of a character.
fn raw(at: &At) -> Option<&Attrs> {
    match at {
        At::Char { raw, .. } => Some(raw),
        _ => None,
    }
}

/// Formats the kept characters whose marks change: `pairs` maps an old
/// atom to the new one it stays as. A mark goes when the Markdown shows it
/// gone; whitespace, whose emphasis and link the Markdown cannot show at
/// the edge of a mark, only gains marks. Neighbouring characters with the
/// same change are formatted as one range.
fn format(
    txn: &mut TransactionMut,
    at: &[At],
    old: &[Atom],
    new: &[Atom],
    pairs: &[(usize, usize)],
) -> Result<(), Refused> {
    let mut changes: Vec<(XmlTextRef, u32, u32, Attrs)> = Vec::new();
    for &(o, n) in pairs {
        let (Atom::Char(c, before), Atom::Char(_, after)) = (&old[o], &new[n]) else {
            continue;
        };
        let gone: Marks = shown(*c, before)
            .difference(&shown(*c, after))
            .cloned()
            .collect();
        let added: Marks = after.difference(before).cloned().collect();
        if gone.is_empty() && added.is_empty() {
            continue;
        }
        let At::Char {
            text,
            offset,
            len,
            raw,
            ..
        } = &at[o]
        else {
            return Err(Refused);
        };
        let mut change: Attrs = HashMap::new();
        for key in raw.keys() {
            if shown_mark(key, raw).is_some_and(|mark| gone.contains(&mark)) {
                change.insert(key.clone(), Any::Null);
            }
        }
        for mark in &added {
            let (key, value) = mark_value(mark, Some(raw));
            change.insert(key, value);
        }
        match changes.last_mut() {
            Some((t, start, length, attrs))
                if t == text && *start + *length == *offset && *attrs == change =>
            {
                *length += len;
            }
            _ => changes.push((text.clone(), *offset, *len, change)),
        }
    }
    for (text, offset, len, change) in changes {
        text.format(txn, offset, len, change);
    }
    Ok(())
}

/// The shown mark a formatting attribute stands for.
fn shown_mark(key: &str, raw: &Attrs) -> Option<Mark> {
    let value = raw.get(key)?;
    if matches!(value, Any::Null | Any::Undefined | Any::Bool(false)) {
        return None;
    }
    Some(match mark_name(key) {
        "bold" => Mark::Bold,
        "italic" => Mark::Italic,
        "strike" => Mark::Strike,
        "code" => Mark::Code,
        "link" => match value {
            Any::Map(map) => match map.get("href") {
                Some(Any::String(href)) => Mark::Link(href.to_string()),
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    })
}

/// The formatting attribute that writes `mark`: the one `raw` holds for
/// it when there is one, so a link keeps its other attributes.
fn mark_value(mark: &Mark, raw: Option<&Attrs>) -> (Arc<str>, Any) {
    if let Some(raw) = raw {
        for key in raw.keys() {
            if shown_mark(key, raw).as_ref() == Some(mark) {
                return (key.clone(), raw[key].clone());
            }
        }
    }
    let value = match mark {
        Mark::Link(href) => {
            let mut attrs = HashMap::new();
            attrs.insert("href".to_owned(), Any::from(href.as_str()));
            Any::from(attrs)
        }
        _ => Any::from(HashMap::<String, Any>::new()),
    };
    (Arc::from(mark.name()), value)
}

/// The formatting attributes of a new character with `marks`, written
/// next to a character with `source`'s attributes.
fn attrs_for(marks: &Marks, source: Option<&Attrs>) -> Attrs {
    let mut out: Attrs = HashMap::new();
    if let Some(source) = source {
        for (key, value) in source {
            if matches!(mark_name(key), "underline" | "highlight") {
                out.insert(key.clone(), value.clone());
            }
        }
    }
    for mark in marks {
        let (key, value) = mark_value(mark, source);
        out.insert(key, value);
    }
    out
}

/// Where inline content goes: into a text at an offset, or before a
/// child of the block.
enum Cursor {
    Text(XmlTextRef, u32, u32),
    Before(u32),
}

/// The length of `text` in UTF-16 units.
fn length(txn: &TransactionMut, text: &XmlTextRef) -> u32 {
    text.diff(txn, YChange::identity)
        .iter()
        .map(|chunk| match &chunk.insert {
            Out::Any(Any::String(s)) => s.encode_utf16().count() as u32,
            _ => 1,
        })
        .sum()
}

/// What [`insert`] moved: the text that was there before it wrote
/// characters into, with how many UTF-16 units, and how many children it
/// added to the block.
struct Moved {
    into: Option<(XmlTextRef, u32)>,
    children: u32,
}

/// Writes `atoms` into `element` at `cursor`.
fn insert(
    txn: &mut TransactionMut,
    element: &XmlElementRef,
    mut cursor: Cursor,
    atoms: &[Atom],
    source: Option<&Attrs>,
) -> Result<Moved, Refused> {
    let mut moved = Moved {
        into: None,
        children: 0,
    };
    let mut k = 0;
    while k < atoms.len() {
        match &atoms[k] {
            Atom::Char(_, marks) => {
                let mut run = String::new();
                while let Some(Atom::Char(c, m)) = atoms.get(k)
                    && m == marks
                {
                    run.push(*c);
                    k += 1;
                }
                let (text, child, offset, fresh) = match cursor {
                    Cursor::Text(text, child, offset) => (text, child, offset, false),
                    Cursor::Before(child) => match element.get(txn, child) {
                        Some(XmlOut::Text(text)) => (text, child, 0, false),
                        _ => {
                            moved.children += 1;
                            let text = element.insert(txn, child, XmlTextPrelim::new(""));
                            (text, child, 0, true)
                        }
                    },
                };
                text.insert_with_attributes(txn, offset, &run, attrs_for(marks, source));
                let units = run.encode_utf16().count() as u32;
                if !fresh {
                    match &mut moved.into {
                        Some((into, inserted)) if *into == text => *inserted += units,
                        _ => moved.into = Some((text.clone(), units)),
                    }
                }
                let offset = offset + units;
                cursor = Cursor::Text(text, child, offset);
            }
            Atom::Node(block) => {
                let at = match &cursor {
                    Cursor::Before(child) => *child,
                    Cursor::Text(_, child, 0) => *child,
                    Cursor::Text(text, child, offset) if *offset == length(txn, text) => child + 1,
                    // A node inside a text would split it: not yet.
                    Cursor::Text(..) => return Err(Refused),
                };
                write_block(txn, element, at, block)?;
                moved.children += 1;
                cursor = match cursor {
                    Cursor::Text(text, child, 0) if at == child => Cursor::Text(text, child + 1, 0),
                    _ => Cursor::Before(at + 1),
                };
                k += 1;
            }
            Atom::Opaque => return Err(Refused),
        }
    }
    Ok(moved)
}

/// Writes `blocks` into `parent` from `at` on.
pub(super) fn write_blocks(
    txn: &mut TransactionMut,
    parent: &impl XmlFragment,
    at: u32,
    blocks: &[Block],
) -> Result<(), Refused> {
    for (k, block) in blocks.iter().enumerate() {
        write_block(txn, parent, at + k as u32, block)?;
    }
    Ok(())
}

fn write_block(
    txn: &mut TransactionMut,
    parent: &impl XmlFragment,
    at: u32,
    block: &Block,
) -> Result<(), Refused> {
    let element = parent.insert(txn, at, XmlElementPrelim::empty(block.name.as_str()));
    for (key, value) in &block.attrs {
        element.insert_attribute(txn, *key, any(key, value)?);
    }
    match &block.content {
        Content::Blocks(children) => write_blocks(txn, &element, 0, children),
        Content::Inline(atoms) => {
            insert(txn, &element, Cursor::Before(0), atoms, None)?;
            Ok(())
        }
        Content::Leaf => Ok(()),
        Content::Opaque => Err(Refused),
    }
}
