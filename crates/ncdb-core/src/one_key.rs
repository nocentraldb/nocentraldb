//! Which of the keys a member holds for one generation the generation's
//! own control change opens under (Carsten, 2026-11-20, NOC-389). A member
//! can hold several keys for a generation when a key envelope to it holds
//! a wrong key; the one this finds is the only one it then opens sealed
//! changes with.
//!
//! A key counts only when it is the one key that opens the change. The
//! core's AEADs do not commit to their key, so the change's author could
//! build it to open under two keys (RFC 0003, "Primitives"); it then
//! confirms none.

use crate::keys::SpaceKey;

/// The one key of `keys` under which `open` gives a value, with that value;
/// `None` when no key or more than one does.
pub(crate) fn only_one<'k, T>(
    keys: &[&'k SpaceKey],
    mut open: impl FnMut(&SpaceKey) -> Option<T>,
) -> Option<(&'k SpaceKey, T)> {
    let mut opened = None;
    for &key in keys {
        if let Some(value) = open(key) {
            if opened.is_some() {
                return None;
            }
            opened = Some((key, value));
        }
    }
    opened
}

#[cfg(test)]
mod tests {
    // On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
    // this routes #[test] there, so every test runs on both targets (NOC-10).
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::only_one;
    use crate::keys::SpaceKey;

    fn keys() -> [SpaceKey; 3] {
        [1, 2, 3].map(|n| SpaceKey::from_bytes([n; 32]))
    }

    /// An opener that opens under the keys whose first byte is in `opens`.
    fn opener(opens: &[u8]) -> impl FnMut(&SpaceKey) -> Option<u8> + '_ {
        |key| opens.contains(&key.bytes()[0]).then_some(key.bytes()[0])
    }

    #[test]
    fn the_one_key_that_opens_gives_its_value_in_any_position() {
        let keys = keys();
        for n in 1..=3 {
            let held: Vec<&SpaceKey> = keys.iter().collect();
            let (key, value) = only_one(&held, opener(&[n])).expect("one key opens");
            assert_eq!((key.bytes()[0], value), (n, n));
        }
    }

    #[test]
    fn no_key_that_opens_gives_nothing() {
        let keys = keys();
        let held: Vec<&SpaceKey> = keys.iter().collect();
        assert!(only_one(&held, opener(&[])).is_none());
        assert!(only_one(&[], opener(&[1])).is_none());
    }

    #[test]
    fn two_keys_that_open_give_nothing() {
        let keys = keys();
        let held: Vec<&SpaceKey> = keys.iter().collect();
        assert!(only_one(&held, opener(&[1, 3])).is_none());
    }
}
