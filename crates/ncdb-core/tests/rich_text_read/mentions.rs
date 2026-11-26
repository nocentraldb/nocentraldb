//! The mentions read (RFC 0013, "Mentions"): the references the mention
//! nodes hold, `space` and `id`, for an app that resolves names.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::rich_text::read::{Mention, markdown, mentions};

use crate::doc::{N, el, p, s, state, t};

const SPACE: &str = "0123456789abcdef0123456789abcdef";
const ID: &str = "fedcba9876543210fedcba9876543210";
const OTHER: &str = "00000000000000000000000000000001";

fn mention(space: &str, id: &str) -> N {
    el("mention", &[("space", s(space)), ("id", s(id))], vec![])
}

fn hex(text: &str) -> [u8; 16] {
    let mut out = [0; 16];
    for (n, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * n..2 * n + 2], 16).unwrap();
    }
    out
}

#[test]
fn a_mention_reads_as_a_link_without_a_name() {
    let state = state(vec![p(vec![t("Ask "), mention(SPACE, ID), t(".")])]);
    assert_eq!(markdown(&state), format!("Ask [@](ncdb:{SPACE}/{ID})."));
}

#[test]
fn mentions_lists_each_reference_in_document_order() {
    let state = state(vec![
        p(vec![mention(SPACE, ID), t(" and "), mention(SPACE, OTHER)]),
        el(
            "bulletList",
            &[],
            vec![el("listItem", &[], vec![p(vec![mention(SPACE, ID)])])],
        ),
    ]);
    let reference = |id: &str| Mention {
        space: SpaceId(hex(SPACE)),
        id: ObjectId(hex(id)),
    };
    assert_eq!(
        mentions(&state),
        vec![reference(ID), reference(OTHER), reference(ID)]
    );
}

#[test]
fn a_mention_without_two_ids_reads_as_a_tag_and_is_no_reference() {
    // TipTap's own Mention extension writes an id and the name as typed.
    let tiptap = el("mention", &[("id", s("ada")), ("label", s("Ada"))], vec![]);
    let upper = mention(&SPACE.to_uppercase(), ID);
    let short = mention(SPACE, "fedcba");
    let state = state(vec![p(vec![tiptap, upper, short])]);
    assert_eq!(mentions(&state), vec![]);
    assert_eq!(
        markdown(&state),
        format!(
            "<mention id=\"ada\" label=\"Ada\"></mention>\
             <mention id=\"{ID}\" space=\"{}\"></mention>\
             <mention id=\"fedcba\" space=\"{SPACE}\"></mention>",
            SPACE.to_uppercase()
        )
    );
}
