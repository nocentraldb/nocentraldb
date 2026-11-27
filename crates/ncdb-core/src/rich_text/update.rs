//! The core's own reading of a Yjs update in encoding v1 (RFC 0013, "How an
//! edit travels in a change"), before Yrs sees it (NOC-468, B): which ids
//! the update adds and which its delete set names, and a refusal of every
//! update that does not follow the format.
//!
//! The format is lib0's: a list of clients, each with its structs in clock
//! order, then the delete set. On top of what the format itself requires,
//! the check holds every update to what an honest writer produces, so that
//! the core and Yrs never read one update two ways:
//!
//! - every count fits in the bytes that are left, and nothing follows the
//!   delete set;
//! - a client id fits 53 bits, a JavaScript number;
//! - every clock, and every clock plus its length, stays below 2^30, more
//!   than any document's writer reaches;
//! - every other number fits 32 bits, every text is UTF-8, and a value
//!   nests at most 64 deep;
//! - a struct is never empty, a client is listed once in the structs and
//!   once in the delete set, and a struct only names ids its writer had
//!   written before it: an origin, a right origin or a parent of its own
//!   client lies below its own clock.
//!
//! Each run of clocks an update adds carries a digest of what it adds,
//! however its items are cut into pieces, so that two updates can be told
//! apart when they give one id different content (RFC 0013, "The same id
//! twice").

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

/// The largest client id: Yjs keeps client ids in JavaScript numbers.
const MAX_CLIENT: u64 = (1 << 53) - 1;
/// Every clock and every end of a range stays below this.
const CLOCK_LIMIT: u64 = 1 << 30;
/// How deeply a value may nest.
const MAX_DEPTH: usize = 64;

/// A range of ids of one client: `len` clocks from `clock`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Range {
    /// The Yjs client id.
    pub client: u64,
    /// The first clock.
    pub clock: u64,
    /// How many clocks.
    pub len: u64,
}

impl Range {
    /// The clock after the range.
    pub fn end(&self) -> u64 {
        self.clock + self.len
    }
}

/// A run of ids an update adds, with a digest of what it adds there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Run {
    /// The ids.
    pub range: Range,
    /// SHA-256, cut to 16 bytes, of the run's content, the same however the
    /// update cuts it into items.
    pub digest: [u8; 16],
}

/// What a well-formed update adds and deletes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    /// The runs of ids it adds, by client, in clock order.
    pub runs: Vec<Run>,
    /// The ids its delete set names, by client, in clock order, merged.
    pub deletes: Vec<Range>,
}

/// An update that does not follow the format. It names nothing of the
/// update, which is content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

/// Reads `bytes`, a Yjs update in encoding v1.
pub fn read(bytes: &[u8]) -> Result<Shape, Malformed> {
    read_with_structs(bytes).map(|(shape, _)| shape)
}

/// The structs of `bytes`, a well-formed update, without its delete set:
/// the bytes before the delete set.
pub(super) fn structs(bytes: &[u8]) -> Result<&[u8], Malformed> {
    read_with_structs(bytes).map(|(_, end)| &bytes[..end])
}

/// [`read`], and where the update's delete set starts.
fn read_with_structs(bytes: &[u8]) -> Result<(Shape, usize), Malformed> {
    let mut cursor = Cursor { bytes, at: 0 };
    let mut shape = Shape::default();
    let mut clients = BTreeSet::new();
    for _ in 0..cursor.count()? {
        let structs = cursor.count()?;
        let client = cursor.client()?;
        if structs == 0 || !clients.insert(client) {
            return Err(Malformed);
        }
        let mut writer = Writer::new(client, cursor.clock()?);
        for _ in 0..structs {
            read_struct(&mut cursor, &mut writer)?;
        }
        shape.runs.extend(writer.finish());
    }
    let end = cursor.at;
    shape.deletes = read_deletes(&mut cursor)?;
    match cursor.at == bytes.len() {
        true => Ok((shape, end)),
        false => Err(Malformed),
    }
}

fn read_deletes(cursor: &mut Cursor<'_>) -> Result<Vec<Range>, Malformed> {
    let mut deletes: Vec<Range> = Vec::new();
    let mut clients = BTreeSet::new();
    for _ in 0..cursor.count()? {
        let client = cursor.client()?;
        let ranges = cursor.count()?;
        if ranges == 0 || !clients.insert(client) {
            return Err(Malformed);
        }
        for _ in 0..ranges {
            let clock = cursor.clock()?;
            let len = cursor.length(clock)?;
            deletes.push(Range { client, clock, len });
        }
    }
    deletes.sort();
    let mut merged: Vec<Range> = Vec::with_capacity(deletes.len());
    for range in deletes {
        match merged.last_mut() {
            Some(last) if last.client == range.client && range.clock <= last.end() => {
                last.len = last.len.max(range.end() - last.clock);
            }
            _ => merged.push(range),
        }
    }
    Ok(merged)
}

/// An id an item names: its origin, its right origin or its parent.
type Id = (u64, u64);

/// Where an item sits, as the encoding states it.
#[derive(Clone, PartialEq, Eq)]
struct Place {
    origin: Option<Id>,
    right: Option<Id>,
    /// The parent, a root's name or an id, and the key in it, when the
    /// item states them: only without origins.
    parent: Option<(Vec<u8>, Option<Vec<u8>>)>,
}

/// The kinds of content: lib0's numbers, GC as 0.
const GC: u8 = 0;
const DELETED: u8 = 1;
const JSON: u8 = 2;
const BINARY: u8 = 3;
const STRING: u8 = 4;
const EMBED: u8 = 5;
const FORMAT: u8 = 6;
const TYPE: u8 = 7;
const ANY: u8 = 8;
const DOC: u8 = 9;
const SKIP: u8 = 10;

const HAS_ORIGIN: u8 = 0x80;
const HAS_RIGHT_ORIGIN: u8 = 0x40;
const HAS_PARENT_SUB: u8 = 0x20;
const KIND: u8 = 0x1f;

fn read_struct(cursor: &mut Cursor<'_>, writer: &mut Writer) -> Result<(), Malformed> {
    let info = cursor.byte()?;
    let kind = info & KIND;
    if kind == GC || kind == SKIP {
        if info != kind {
            return Err(Malformed);
        }
        let len = cursor.length(writer.clock)?;
        return match kind {
            GC => writer.add(Place::none(), GC, &[], len),
            _ => writer.skip(len),
        };
    }
    if kind > DOC {
        return Err(Malformed);
    }
    let own = |id: Id| id.0 == writer.client && id.1 >= writer.clock;
    let origin = (info & HAS_ORIGIN != 0).then(|| cursor.id()).transpose()?;
    let right = (info & HAS_RIGHT_ORIGIN != 0)
        .then(|| cursor.id())
        .transpose()?;
    let mut parent = None;
    if origin.is_none() && right.is_none() {
        let mut named = Vec::new();
        match cursor.uint32()? {
            1 => named.extend(cursor.string()?.as_bytes()),
            0 => {
                let id = cursor.id()?;
                if own(id) {
                    return Err(Malformed);
                }
                named.extend(id.0.to_be_bytes());
                named.extend(id.1.to_be_bytes());
            }
            _ => return Err(Malformed),
        }
        let sub = (info & HAS_PARENT_SUB != 0)
            .then(|| cursor.string().map(|s| s.as_bytes().to_vec()))
            .transpose()?;
        parent = Some((named, sub));
    }
    if origin.is_some_and(own) || right.is_some_and(own) {
        return Err(Malformed);
    }
    // What the digest takes of the content: for text, JSON and values the
    // units without the count in front, so that a cut changes nothing.
    let mut start = cursor.at;
    let len = match kind {
        DELETED => cursor.length(writer.clock)?,
        JSON => {
            let n = cursor.count()?;
            start = cursor.at;
            for _ in 0..n {
                cursor.string()?;
            }
            n
        }
        BINARY => cursor.take_counted().map(|_| 1)?,
        STRING => {
            let text = cursor.string()?;
            start = cursor.at - text.len();
            text.encode_utf16().count() as u64
        }
        EMBED => cursor.string().map(|_| 1)?,
        FORMAT => cursor.string().and_then(|_| cursor.string()).map(|_| 1)?,
        TYPE => match cursor.uint32()? {
            3 | 5 => cursor.string().map(|_| 1)?,
            0..=6 => 1,
            _ => return Err(Malformed),
        },
        ANY => {
            let n = cursor.count()?;
            start = cursor.at;
            for _ in 0..n {
                cursor.any(0)?;
            }
            n
        }
        _ => cursor.string().and_then(|_| cursor.any(0)).map(|_| 1)?,
    };
    if len == 0 || writer.clock + len >= CLOCK_LIMIT {
        return Err(Malformed);
    }
    let content = match kind {
        DELETED => &[][..],
        _ => &cursor.bytes[start..cursor.at],
    };
    writer.add(
        Place {
            origin,
            right,
            parent,
        },
        kind,
        content,
        len,
    )
}

impl Place {
    fn none() -> Place {
        Place {
            origin: None,
            right: None,
            parent: None,
        }
    }
}

/// The runs one client's structs add, with their digests.
///
/// A run's digest covers its segments. A segment is a stretch of units of
/// one kind that each continue the one before: its origin is the unit just
/// before, and it has the same right origin; GC units always continue. However an update cuts items,
/// a piece after a cut continues the piece before it, so the segments, and
/// the digest, stay the same.
struct Writer {
    client: u64,
    clock: u64,
    run: Option<(u64, Sha256)>,
    segment: Option<Segment>,
    runs: Vec<Run>,
}

struct Segment {
    kind: u8,
    right: Option<Id>,
    units: u64,
    content: Sha256,
}

impl Writer {
    fn new(client: u64, clock: u64) -> Writer {
        Writer {
            client,
            clock,
            run: None,
            segment: None,
            runs: Vec::new(),
        }
    }

    fn add(&mut self, place: Place, kind: u8, content: &[u8], len: u64) -> Result<(), Malformed> {
        let previous = self.clock.checked_sub(1).map(|clock| (self.client, clock));
        let continues = self.segment.as_ref().is_some_and(|segment| {
            segment.kind == kind
                && (kind == GC
                    || segment.right == place.right
                        && place.origin.is_some()
                        && place.origin == previous)
        });
        if !continues {
            self.close_segment();
            let run = &mut self
                .run
                .get_or_insert_with(|| (self.clock, Sha256::new()))
                .1;
            for id in [place.origin, place.right] {
                match id {
                    Some((client, clock)) => {
                        run.update([1]);
                        run.update(client.to_be_bytes());
                        run.update(clock.to_be_bytes());
                    }
                    None => run.update([0]),
                }
            }
            match &place.parent {
                Some((named, sub)) => {
                    run.update([1]);
                    hash_counted(run, named);
                    match sub {
                        Some(sub) => {
                            run.update([1]);
                            hash_counted(run, sub);
                        }
                        None => run.update([0]),
                    }
                }
                None => run.update([0]),
            }
            self.segment = Some(Segment {
                kind,
                right: place.right,
                units: 0,
                content: Sha256::new(),
            });
        }
        let segment = self.segment.as_mut().expect("a segment is open");
        segment.units += len;
        segment.content.update(content);
        self.clock += len;
        Ok(())
    }

    fn skip(&mut self, len: u64) -> Result<(), Malformed> {
        self.close_run();
        self.clock += len;
        Ok(())
    }

    fn close_segment(&mut self) {
        if let (Some(segment), Some((_, run))) = (self.segment.take(), self.run.as_mut()) {
            run.update([segment.kind]);
            run.update(segment.units.to_be_bytes());
            run.update(segment.content.finalize());
        }
    }

    fn close_run(&mut self) {
        self.close_segment();
        if let Some((start, run)) = self.run.take() {
            let digest = run.finalize();
            self.runs.push(Run {
                range: Range {
                    client: self.client,
                    clock: start,
                    len: self.clock - start,
                },
                digest: digest[..16].try_into().expect("16 of 32 bytes"),
            });
        }
    }

    fn finish(mut self) -> Vec<Run> {
        self.close_run();
        self.runs
    }
}

fn hash_counted(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}

/// lib0's tags of a value (`any`).
const UNDEFINED: u8 = 127;
const NULL: u8 = 126;
const INTEGER: u8 = 125;
const FLOAT32: u8 = 124;
const FLOAT64: u8 = 123;
const BIGINT: u8 = 122;
const FALSE: u8 = 121;
const TRUE: u8 = 120;
const TEXT: u8 = 119;
const OBJECT: u8 = 118;
const ARRAY: u8 = 117;
const BYTES: u8 = 116;

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn left(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn byte(&mut self) -> Result<u8, Malformed> {
        let byte = *self.bytes.get(self.at).ok_or(Malformed)?;
        self.at += 1;
        Ok(byte)
    }

    fn take(&mut self, n: u64) -> Result<&'a [u8], Malformed> {
        let n = usize::try_from(n).map_err(|_| Malformed)?;
        if n > self.left() {
            return Err(Malformed);
        }
        let taken = &self.bytes[self.at..self.at + n];
        self.at += n;
        Ok(taken)
    }

    /// lib0's unsigned varint, at most ten bytes, no overflow.
    fn uint(&mut self) -> Result<u64, Malformed> {
        let mut value: u64 = 0;
        for shift in (0..70).step_by(7) {
            let byte = self.byte()?;
            let bits = u64::from(byte & 0x7f);
            if shift == 63 && bits > 1 {
                return Err(Malformed);
            }
            value |= bits << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(Malformed)
    }

    fn uint32(&mut self) -> Result<u64, Malformed> {
        match self.uint()? {
            n if n <= u64::from(u32::MAX) => Ok(n),
            _ => Err(Malformed),
        }
    }

    /// lib0's signed varint: a sign bit and six bits, then seven per byte.
    fn int(&mut self) -> Result<(), Malformed> {
        let mut byte = self.byte()?;
        for _ in 0..9 {
            if byte & 0x80 == 0 {
                return Ok(());
            }
            byte = self.byte()?;
        }
        Err(Malformed)
    }

    /// A count of things that each take at least one byte.
    fn count(&mut self) -> Result<u64, Malformed> {
        match self.uint()? {
            n if n <= self.left() as u64 => Ok(n),
            _ => Err(Malformed),
        }
    }

    fn client(&mut self) -> Result<u64, Malformed> {
        match self.uint()? {
            n if n <= MAX_CLIENT => Ok(n),
            _ => Err(Malformed),
        }
    }

    fn clock(&mut self) -> Result<u64, Malformed> {
        match self.uint()? {
            n if n < CLOCK_LIMIT => Ok(n),
            _ => Err(Malformed),
        }
    }

    /// A length of at least one that ends below the limit from `clock`.
    fn length(&mut self, clock: u64) -> Result<u64, Malformed> {
        match self.uint()? {
            n if (1..CLOCK_LIMIT).contains(&n) && clock + n < CLOCK_LIMIT => Ok(n),
            _ => Err(Malformed),
        }
    }

    fn id(&mut self) -> Result<Id, Malformed> {
        Ok((self.client()?, self.clock()?))
    }

    fn take_counted(&mut self) -> Result<&'a [u8], Malformed> {
        let n = self.uint32()?;
        self.take(n)
    }

    fn string(&mut self) -> Result<&'a str, Malformed> {
        let bytes = self.take_counted()?;
        std::str::from_utf8(bytes).map_err(|_| Malformed)
    }

    fn any(&mut self, depth: usize) -> Result<(), Malformed> {
        if depth >= MAX_DEPTH {
            return Err(Malformed);
        }
        match self.byte()? {
            UNDEFINED | NULL | FALSE | TRUE => Ok(()),
            INTEGER => self.int(),
            FLOAT32 => self.take(4).map(|_| ()),
            FLOAT64 | BIGINT => self.take(8).map(|_| ()),
            TEXT => self.string().map(|_| ()),
            BYTES => self.take_counted().map(|_| ()),
            OBJECT => {
                for _ in 0..self.count()? {
                    self.string()?;
                    self.any(depth + 1)?;
                }
                Ok(())
            }
            ARRAY => {
                for _ in 0..self.count()? {
                    self.any(depth + 1)?;
                }
                Ok(())
            }
            _ => Err(Malformed),
        }
    }
}
