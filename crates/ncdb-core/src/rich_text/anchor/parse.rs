//! Markdown read back as blocks (RFC 0013, "An agent: anchored edits",
//! step 3): CommonMark with GitHub's tables, strikethrough and task lists,
//! into TipTap's standard vocabulary, the names the core's rendering
//! writes. A mention's link form `[@](ncdb:<space id>/<object id>)` reads
//! as a mention; for a first document, any other `ncdb:` link is refused
//! (NOC-536). HTML, images and anything else the rendering never writes
//! read as nothing: the edit is refused.

use std::collections::BTreeMap;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::view::{Atom, Block, Content, Mark, drops_empty, is_id};

/// The Markdown extensions the rendering writes.
pub(super) fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// The blocks `markdown` reads as, or `None` when it holds something the
/// core does not write from Markdown.
pub(super) fn parse(markdown: &str) -> Option<Vec<Block>> {
    read(markdown, false)
}

/// The blocks of a first document `markdown` reads as: as [`parse`], and
/// `None` too when it holds an `ncdb:` link that is no mention's form, such
/// as one without both ids, so that a malformed mention is refused instead
/// of written as a plain link.
pub(super) fn parse_first(markdown: &str) -> Option<Vec<Block>> {
    read(markdown, true)
}

fn read(markdown: &str, refuse_ncdb_links: bool) -> Option<Vec<Block>> {
    let mut reader = Reader {
        frames: vec![Frame::blocks("doc", BTreeMap::new())],
        marks: Vec::new(),
        links: Vec::new(),
        header: false,
        refuse_ncdb_links,
    };
    for event in Parser::new_ext(markdown, options()) {
        reader.event(event)?;
    }
    reader.close_implicit();
    let root = reader.frames.pop()?;
    match (reader.frames.is_empty(), root.kind) {
        (true, Kind::Blocks(blocks)) => Some(blocks),
        _ => None,
    }
}

struct Frame {
    name: String,
    attrs: BTreeMap<&'static str, String>,
    kind: Kind,
}

enum Kind {
    Blocks(Vec<Block>),
    /// Inline content; `implicit` when no paragraph tag opened it, as in a
    /// tight list item or a table cell.
    Inline {
        atoms: Vec<Atom>,
        implicit: bool,
    },
}

impl Frame {
    fn blocks(name: &str, attrs: BTreeMap<&'static str, String>) -> Frame {
        Frame {
            name: name.to_owned(),
            attrs,
            kind: Kind::Blocks(Vec::new()),
        }
    }

    fn inline(name: &str, attrs: BTreeMap<&'static str, String>, implicit: bool) -> Frame {
        Frame {
            name: name.to_owned(),
            attrs,
            kind: Kind::Inline {
                atoms: Vec::new(),
                implicit,
            },
        }
    }

    fn block(self) -> Block {
        let content = match self.kind {
            Kind::Blocks(blocks) => Content::Blocks(blocks),
            Kind::Inline { mut atoms, .. } => {
                // A fenced code block's text ends with the line break
                // before its closing fence.
                if self.name == "codeBlock" && matches!(atoms.last(), Some(Atom::Char('\n', _))) {
                    atoms.pop();
                }
                Content::Inline(atoms)
            }
        };
        Block {
            name: self.name,
            attrs: self.attrs,
            content,
        }
    }
}

struct Reader {
    frames: Vec<Frame>,
    marks: Vec<Mark>,
    /// For each open link, its destination and where its text starts.
    links: Vec<(String, usize)>,
    header: bool,
    /// Whether an `ncdb:` link that reads as no mention refuses the
    /// Markdown.
    refuse_ncdb_links: bool,
}

impl Reader {
    fn event(&mut self, event: Event<'_>) -> Option<()> {
        match event {
            Event::Start(tag) => self.start(tag)?,
            Event::End(tag) => self.end(tag)?,
            Event::Text(text) => {
                let code = self.top().name == "codeBlock";
                let marks = if code { Vec::new() } else { self.marks.clone() };
                for c in text.chars() {
                    self.push_atom(Atom::Char(c, marks.iter().cloned().collect()));
                }
            }
            Event::Code(text) => {
                let mut marks: super::view::Marks = self.marks.iter().cloned().collect();
                marks.insert(Mark::Code);
                for c in text.chars() {
                    self.push_atom(Atom::Char(c, marks.clone()));
                }
            }
            Event::SoftBreak => {
                let marks = self.marks.iter().cloned().collect();
                self.push_atom(Atom::Char(' ', marks));
            }
            Event::HardBreak => self.push_atom(Atom::Node(leaf("hardBreak", BTreeMap::new()))),
            Event::Rule => self.push_block(leaf("horizontalRule", BTreeMap::new())),
            Event::TaskListMarker(checked) => {
                let item = self
                    .frames
                    .iter_mut()
                    .rev()
                    .find(|f| f.name == "listItem")?;
                item.name = "taskItem".to_owned();
                item.attrs.insert("checked", checked.to_string());
            }
            _ => return None,
        }
        Some(())
    }

    fn start(&mut self, tag: Tag<'_>) -> Option<()> {
        let none = BTreeMap::new;
        match tag {
            Tag::Paragraph => self.open(Frame::inline("paragraph", none(), false)),
            Tag::Heading { level, .. } => {
                let attrs = BTreeMap::from([("level", (level as u8).to_string())]);
                self.open(Frame::inline("heading", attrs, false));
            }
            Tag::CodeBlock(kind) => {
                let mut attrs = none();
                if let CodeBlockKind::Fenced(language) = kind
                    && !language.is_empty()
                {
                    attrs.insert("language", language.to_string());
                }
                self.open(Frame::inline("codeBlock", attrs, false));
            }
            Tag::BlockQuote(None) => self.open(Frame::blocks("blockquote", none())),
            Tag::List(None) => self.open(Frame::blocks("bulletList", none())),
            Tag::List(Some(start)) => {
                let attrs = BTreeMap::from([("start", start.to_string())]);
                self.open(Frame::blocks("orderedList", attrs));
            }
            Tag::Item => self.open(Frame::blocks("listItem", none())),
            Tag::Table(_) => self.open(Frame::blocks("table", none())),
            Tag::TableHead => {
                self.header = true;
                self.open(Frame::blocks("tableRow", none()));
            }
            Tag::TableRow => self.open(Frame::blocks("tableRow", none())),
            Tag::TableCell => {
                let name = if self.header {
                    "tableHeader"
                } else {
                    "tableCell"
                };
                self.open(Frame::blocks(name, none()));
                self.frames.push(Frame::inline("paragraph", none(), true));
            }
            Tag::Emphasis => self.marks.push(Mark::Italic),
            Tag::Strong => self.marks.push(Mark::Bold),
            Tag::Strikethrough => self.marks.push(Mark::Strike),
            Tag::Link { dest_url, .. } => {
                let at = match &self.top().kind {
                    Kind::Inline { atoms, .. } => atoms.len(),
                    Kind::Blocks(_) => 0,
                };
                self.links.push((dest_url.to_string(), at));
                self.marks.push(Mark::Link(dest_url.to_string()));
            }
            _ => return None,
        }
        Some(())
    }

    fn end(&mut self, tag: TagEnd) -> Option<()> {
        match tag {
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.marks.pop()?;
            }
            TagEnd::Link => {
                self.marks.pop()?;
                let (href, at) = self.links.pop()?;
                if !self.mention(&href, at) && self.refuse_ncdb_links && is_ncdb(&href) {
                    return None;
                }
            }
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock => {
                let frame = self.frames.pop()?;
                self.push_block(frame.block());
            }
            TagEnd::TableHead => {
                self.header = false;
                self.close()?;
            }
            TagEnd::BlockQuote(_)
            | TagEnd::Item
            | TagEnd::Table
            | TagEnd::TableRow
            | TagEnd::TableCell => self.close()?,
            TagEnd::List(_) => {
                self.close_implicit();
                let mut frame = self.frames.pop()?;
                if frame.name == "bulletList"
                    && let Kind::Blocks(items) = &frame.kind
                    && items.iter().any(|item| item.name == "taskItem")
                {
                    frame.name = "taskList".to_owned();
                }
                self.push_block(frame.block());
            }
            _ => return None,
        }
        Some(())
    }

    /// A link whose text is `@` alone and whose destination is
    /// `ncdb:<space id>/<object id>` reads as a mention; whether it did.
    fn mention(&mut self, href: &str, at: usize) -> bool {
        let Some((space, id)) = href
            .strip_prefix("ncdb:")
            .and_then(|ids| ids.split_once('/'))
        else {
            return false;
        };
        if !(is_id(space) && is_id(id)) {
            return false;
        }
        let Kind::Inline { atoms, .. } = &mut self.frames.last_mut().expect("the root").kind else {
            return false;
        };
        if atoms.len() == at + 1 && matches!(atoms[at], Atom::Char('@', _)) {
            atoms[at] = Atom::Node(leaf(
                "mention",
                BTreeMap::from([("space", space.to_owned()), ("id", id.to_owned())]),
            ));
            return true;
        }
        false
    }

    fn top(&self) -> &Frame {
        self.frames.last().expect("the root frame stays")
    }

    /// Opens a block: inline content that no tag opened ends first.
    fn open(&mut self, frame: Frame) {
        self.close_implicit();
        self.frames.push(frame);
    }

    /// Closes the block on top, after inline content that no tag opened.
    fn close(&mut self) -> Option<()> {
        self.close_implicit();
        if self.frames.len() < 2 {
            return None;
        }
        let frame = self.frames.pop()?;
        self.push_block(frame.block());
        Some(())
    }

    fn close_implicit(&mut self) {
        if let Some(Frame {
            kind: Kind::Inline { implicit: true, .. },
            ..
        }) = self.frames.last()
        {
            let frame = self.frames.pop().expect("the frame just seen");
            self.push_block(frame.block());
        }
    }

    fn push_block(&mut self, block: Block) {
        self.close_implicit();
        let top = self.frames.last_mut().expect("the root frame stays");
        let drops = drops_empty(&top.name);
        match &mut top.kind {
            Kind::Blocks(blocks) => {
                if !(drops && block.invisible()) {
                    blocks.push(block);
                }
            }
            // A block inside inline content: no rendering writes one.
            Kind::Inline { atoms, .. } => atoms.push(Atom::Opaque),
        }
    }

    /// Adds inline content, opening a paragraph where blocks stand.
    fn push_atom(&mut self, atom: Atom) {
        if matches!(self.top().kind, Kind::Blocks(_)) {
            self.frames
                .push(Frame::inline("paragraph", BTreeMap::new(), true));
        }
        if let Kind::Inline { atoms, .. } = &mut self.frames.last_mut().expect("a frame").kind {
            atoms.push(atom);
        }
    }
}

/// Whether `href` is of the scheme `ncdb`, in any case, as URL schemes are.
fn is_ncdb(href: &str) -> bool {
    href.get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("ncdb:"))
}

fn leaf(name: &str, attrs: BTreeMap<&'static str, String>) -> Block {
    Block {
        name: name.to_owned(),
        attrs,
        content: Content::Leaf,
    }
}
