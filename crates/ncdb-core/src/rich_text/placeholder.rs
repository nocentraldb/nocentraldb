//! A rejected operation's placeholders (RFC 0013, "Un-folding a rejected
//! change"): the update's items, each deleted, without the update's own
//! delete set. Later edits placed next to the items stay where they are,
//! the text the update deleted is back, and text typed inside a block it
//! added goes with that block.

use super::update::{self, Malformed, Range, Run};

/// The update that places `update`'s items as deleted placeholders: its
/// structs, then a delete set that names exactly the ids it adds.
pub(super) fn update(update: &[u8], runs: &[Run]) -> Result<Vec<u8>, Malformed> {
    let mut bytes = update::structs(update)?.to_vec();
    let mut ranges: Vec<Range> = runs.iter().map(|run| run.range).collect();
    ranges.sort();
    let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if last.client == range.client && range.clock <= last.end() => {
                last.len = last.len.max(range.end() - last.clock);
            }
            _ => merged.push(range),
        }
    }
    let mut clients: Vec<(u64, Vec<Range>)> = Vec::new();
    for range in merged {
        match clients.last_mut() {
            Some((client, ranges)) if *client == range.client => ranges.push(range),
            _ => clients.push((range.client, vec![range])),
        }
    }
    uint(&mut bytes, clients.len() as u64);
    for (client, ranges) in clients {
        uint(&mut bytes, client);
        uint(&mut bytes, ranges.len() as u64);
        for range in ranges {
            uint(&mut bytes, range.clock);
            uint(&mut bytes, range.len);
        }
    }
    Ok(bytes)
}

/// lib0's unsigned variable-length integer.
fn uint(bytes: &mut Vec<u8>, mut n: u64) {
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            bytes.push(byte);
            return;
        }
        bytes.push(byte | 0x80);
    }
}

/// Whether two lists of runs share an id, whatever their content.
pub(super) fn overlap(a: &[Run], b: &[Run]) -> bool {
    a.iter().any(|x| {
        b.iter().any(|y| {
            x.range.client == y.range.client
                && x.range.clock < y.range.end()
                && y.range.clock < x.range.end()
        })
    })
}
