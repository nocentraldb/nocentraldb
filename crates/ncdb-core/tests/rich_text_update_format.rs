//! The core reads a rich-text update's encoding v1 itself before Yrs sees
//! it (NOC-468, B): what the update adds and deletes, and a refusal of any
//! update that does not follow the format, so that only well-formed updates
//! reach Yrs. An update the check refuses parks as "malformed arguments"
//! (RFC 0013, "Folding", step 1).
#![cfg(feature = "rich-text")]

mod rich_text_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::update::{Range, read};
use rich_text_support::{
    Bytes, ROOT, add_block, apply, bold, delete_block, delete_text, doc, set_attribute, state,
    string_item, type_text,
};

fn ranges(update: &[u8]) -> (Vec<Range>, Vec<Range>) {
    let read = read(update).expect("a well-formed update");
    (
        read.runs.iter().map(|run| run.range).collect(),
        read.deletes.clone(),
    )
}

fn range(client: u64, clock: u64, len: u64) -> Range {
    Range { client, clock, len }
}

#[test]
fn an_update_yrs_writes_reads_with_the_clocks_it_adds() {
    let a = doc(7);
    // A paragraph element, its text, and five characters: three items.
    let first = add_block(&a, "paragraph", "Hello");
    assert_eq!(ranges(&first), (vec![range(7, 0, 7)], vec![]));
    let typed = type_text(&a, 0, 5, " world");
    assert_eq!(ranges(&typed), (vec![range(7, 7, 6)], vec![]));
}

#[test]
fn an_update_reads_with_the_ids_its_delete_set_names() {
    let a = doc(7);
    add_block(&a, "paragraph", "Hello");
    let deleted = delete_text(&a, 0, 1, 3);
    assert_eq!(ranges(&deleted), (vec![], vec![range(7, 3, 3)]));
}

#[test]
fn formats_attributes_and_block_deletions_read() {
    let a = doc(7);
    add_block(&a, "heading", "Title");
    add_block(&a, "paragraph", "Body text");
    for update in [
        bold(&a, 1, 0, 4),
        set_attribute(&a, 0, "level", "2"),
        delete_block(&a, 0),
    ] {
        read(&update).expect("a well-formed update");
    }
    read(&state(&a)).expect("a whole document reads");
}

#[test]
fn every_truncation_of_an_update_is_refused() {
    let a = doc(7);
    add_block(&a, "paragraph", "Hello");
    bold(&a, 0, 1, 2);
    let whole = state(&a);
    for end in 0..whole.len() {
        assert!(read(&whole[..end]).is_err(), "truncated at {end}");
    }
}

#[test]
fn a_byte_after_the_delete_set_is_refused() {
    let mut update = string_item(7, 0, "x");
    read(&update).expect("well-formed");
    update.push(0);
    assert!(read(&update).is_err());
}

#[test]
fn a_content_kind_the_format_does_not_define_is_refused() {
    for kind in 11u8..32 {
        let update = Bytes::default()
            .uint(1)
            .uint(1)
            .uint(7)
            .uint(0)
            .byte(kind)
            .uint(1)
            .string(ROOT)
            .uint(0)
            .0;
        assert!(read(&update).is_err(), "kind {kind}");
    }
}

#[test]
fn a_count_larger_than_the_bytes_left_is_refused() {
    // About 416 million clients in five bytes.
    let update = Bytes::default().uint(416_000_000).0;
    assert!(read(&update).is_err());
    let structs = Bytes::default().uint(1).uint(u32::MAX.into()).uint(7).uint(0).0;
    assert!(read(&structs).is_err());
}

#[test]
fn a_length_that_runs_past_the_end_is_refused() {
    let update = Bytes::default()
        .uint(1)
        .uint(1)
        .uint(7)
        .uint(0)
        .byte(4)
        .uint(1)
        .string(ROOT)
        .uint(u64::from(u32::MAX) - 1)
        .raw(b"xy")
        .uint(0)
        .0;
    assert!(read(&update).is_err());
}

#[test]
fn a_client_id_beyond_53_bits_is_refused() {
    read(&string_item((1 << 53) - 1, 0, "x")).expect("the largest client id");
    assert!(read(&string_item(1 << 53, 0, "x")).is_err());
}

#[test]
fn a_clock_beyond_what_any_document_reaches_is_refused() {
    read(&string_item(7, (1 << 30) - 2, "x")).expect("below the limit");
    assert!(read(&string_item(7, (1 << 30) - 1, "x")).is_err());
    assert!(read(&string_item(7, u64::MAX >> 8, "x")).is_err());
}

#[test]
fn a_deletion_beyond_what_any_document_reaches_is_refused() {
    let deletes = |clock: u64, len: u64| {
        Bytes::default()
            .uint(0)
            .uint(1)
            .uint(7)
            .uint(1)
            .uint(clock)
            .uint(len)
            .0
    };
    read(&deletes(5, 3)).expect("well-formed");
    assert!(read(&deletes(5, 0)).is_err(), "an empty range");
    assert!(read(&deletes((1 << 30) - 1, 2)).is_err());
}

#[test]
fn an_empty_item_is_refused() {
    assert!(read(&string_item(7, 0, "")).is_err());
}

#[test]
fn text_that_is_not_utf8_is_refused() {
    let update = Bytes::default()
        .uint(1)
        .uint(1)
        .uint(7)
        .uint(0)
        .byte(4)
        .uint(1)
        .string(ROOT)
        .uint(2)
        .raw(&[0xc3, 0x28])
        .uint(0)
        .0;
    assert!(read(&update).is_err());
}

#[test]
fn a_client_listed_twice_is_refused() {
    let item = |clock: u64| {
        Bytes::default()
            .uint(1)
            .uint(7)
            .uint(clock)
            .byte(4)
            .uint(1)
            .string(ROOT)
            .string("x")
            .0
    };
    let update = Bytes::default()
        .uint(2)
        .raw(&item(0))
        .raw(&item(1))
        .uint(0)
        .0;
    assert!(read(&update).is_err());
}

#[test]
fn an_item_that_names_its_own_or_a_later_clock_of_its_writer_is_refused() {
    let with_origin = |origin_clock: u64| {
        Bytes::default()
            .uint(1)
            .uint(1)
            .uint(7)
            .uint(3)
            .byte(4 | 0x80)
            .uint(7)
            .uint(origin_clock)
            .string("x")
            .uint(0)
            .0
    };
    read(&with_origin(2)).expect("an origin before the item");
    assert!(read(&with_origin(3)).is_err());
    assert!(read(&with_origin(9)).is_err());
}

#[test]
fn values_nested_deeper_than_the_format_allows_are_refused() {
    let nested = |depth: usize| {
        let mut bytes = Bytes::default()
            .uint(1)
            .uint(1)
            .uint(7)
            .uint(0)
            .byte(8)
            .uint(1)
            .string(ROOT)
            .uint(1);
        for _ in 0..depth {
            bytes = bytes.byte(117).uint(1);
        }
        bytes.byte(126).uint(0).0
    };
    read(&nested(16)).expect("16 arrays deep");
    assert!(read(&nested(100_000)).is_err());
}

#[test]
fn equal_content_written_in_other_pieces_has_the_same_digest() {
    // Two updates merged keep their two items; the document that applied
    // them holds the same ids as one item.
    let a = doc(7);
    let first = add_block(&a, "paragraph", "Hello");
    let second = type_text(&a, 0, 5, " world");
    let merged = yrs::merge_updates_v1([first.as_slice(), second.as_slice()]).unwrap();
    let b = doc(8);
    apply(&b, &first);
    apply(&b, &second);
    let pieces = read(&merged).expect("a merged update");
    let whole = read(&state(&b)).expect("a whole document");
    assert_eq!(pieces.runs, whole.runs);
    assert_eq!(whole.runs.len(), 1);
}

#[test]
fn other_content_at_the_same_ids_has_another_digest() {
    let x = read(&string_item(7, 0, "P")).unwrap();
    let y = read(&string_item(7, 0, "Q")).unwrap();
    assert_eq!(x.runs[0].range, y.runs[0].range);
    assert_ne!(x.runs[0].digest, y.runs[0].digest);
}
