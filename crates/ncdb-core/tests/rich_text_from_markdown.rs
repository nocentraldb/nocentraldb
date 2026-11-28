//! Markdown read into a first document (RFC 0013, "The first content";
//! NOC-463): the core reads the Markdown back with the parser of its
//! anchored edits, into TipTap's standard vocabulary, and writes the
//! blocks as one Yjs update under the client id it is given. The cases are
//! the table `vectors/rich-text/from-markdown.json`, which the SDKs share.
#![cfg(ncdb_rich_text)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "rich_text_support/mod.rs"]
mod rich_text_support;

#[path = "rich_text_read/doc.rs"]
#[allow(dead_code)]
mod doc;

use ncdb_core::rich_text::anchor::from_markdown;
use ncdb_core::rich_text::read::{Mention, markdown, mentions, tree};
use ncdb_plugin::Data;
use serde_json::Value as Json;

use rich_text_support::harness::client;

const TABLE: &str = include_str!("vectors/rich-text/from-markdown.json");

fn table(part: &str) -> Vec<Json> {
    let table: Json = serde_json::from_str(TABLE).expect("the table is JSON");
    table[part].as_array().expect("a list of cases").clone()
}

/// A tree of the table as the `Data` the core reads.
fn data(json: &Json) -> Data {
    match json {
        Json::Null => Data::Null,
        Json::Bool(b) => Data::Bool(*b),
        Json::Number(n) => match n.as_i64() {
            Some(n) => Data::Integer(n.into()),
            None => Data::Float(n.as_f64().expect("a number")),
        },
        Json::String(text) => Data::Text(text.clone()),
        Json::Array(items) => Data::Array(items.iter().map(data).collect()),
        Json::Object(entries) => {
            Data::Map(entries.iter().map(|(k, v)| (k.clone(), data(v))).collect())
        }
    }
}

#[test]
fn every_case_of_the_table_reads_as_its_document() {
    for case in table("cases") {
        let name = case["name"].as_str().expect("a name");
        let written = from_markdown(case["markdown"].as_str().expect("Markdown"), client(0))
            .unwrap_or_else(|| panic!("{name}: refused"));
        let state = doc::fold(&[written]);
        assert_eq!(tree(&state), data(&case["tree"]), "{name}");
        assert_eq!(
            markdown(&state),
            case["reads"].as_str().expect("reads"),
            "{name}"
        );
        if let Some(listed) = case.get("mentions") {
            assert_eq!(hexes(&mentions(&state)), refs(listed), "{name}");
        }
    }
}

/// A case's `mentions`, as pairs of space id and object id.
fn refs(listed: &Json) -> Vec<(String, String)> {
    let listed = listed.as_array().expect("a list of mentions");
    listed
        .iter()
        .map(|m| {
            (
                m["space"].as_str().unwrap().into(),
                m["id"].as_str().unwrap().into(),
            )
        })
        .collect()
}

fn hexes(mentions: &[Mention]) -> Vec<(String, String)> {
    let hex = |bytes: &[u8; 16]| bytes.iter().map(|b| format!("{b:02x}")).collect();
    mentions
        .iter()
        .map(|m| (hex(&m.space.0), hex(&m.id.0)))
        .collect()
}

#[test]
fn every_refused_case_of_the_table_writes_nothing() {
    for case in table("refused") {
        let name = case["name"].as_str().expect("a name");
        let markdown = case["markdown"].as_str().expect("Markdown");
        assert_eq!(from_markdown(markdown, client(0)), None, "{name}");
    }
}

#[test]
fn the_document_is_written_under_the_client_id_given() {
    let written = from_markdown("Hello.", client(3)).expect("written");
    let shape = ncdb_core::rich_text::update::read(&written).expect("an update");
    assert!(!shape.runs.is_empty());
    assert!(shape.runs.iter().all(|run| run.range.client == client(3)));
}
