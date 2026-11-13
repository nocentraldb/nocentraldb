//! The core of `@nocentraldb/sdk` in the browser (NOC-176): the Rust SDK
//! compiled to WebAssembly, with one export, [`Core`], that the package's
//! worker calls with each bridge request, CBOR bytes, and that answers
//! with the bridge's CBOR answer (`nocentraldb::bridge`). The crate is
//! never published; the package ships the module built from it (Carsten,
//! 2026-11-12, NOC-274, point 1).

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::rc::Rc;

use nocentraldb::bridge::Bridge;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

/// The worker's core: one bridge, and at most one open database.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct Core {
    bridge: Rc<RefCell<Option<Bridge>>>,
}

impl Default for Core {
    fn default() -> Core {
        Core::new()
    }
}

impl Core {
    /// The answer to `request`. The worker sends one request at a time, so
    /// the bridge is free; a request that arrives while another is
    /// answered is refused rather than run at once.
    pub async fn answer(&self, request: Vec<u8>) -> Vec<u8> {
        answer(self.bridge.clone(), request).await
    }
}

async fn answer(bridge: Rc<RefCell<Option<Bridge>>>, request: Vec<u8>) -> Vec<u8> {
    let taken = bridge.borrow_mut().take();
    let Some(mut held) = taken else {
        return busy(&request);
    };
    let answer = held.handle(&request).await;
    *bridge.borrow_mut() = Some(held);
    answer
}

/// The answer to a request that came while another was answered.
fn busy(request: &[u8]) -> Vec<u8> {
    use nocentraldb::bridge::cbor::{self, Key, Value};
    let id = match cbor::decode(request) {
        Ok(Value::Map(map)) => match map.get(&Key::Text("id".into())) {
            Some(Value::Unsigned(id)) => *id,
            _ => 0,
        },
        _ => 0,
    };
    let entry = |k: &str, v: Value| (Key::Text(k.into()), v);
    cbor::encode(&Value::Map(
        [
            entry("id", Value::Unsigned(id)),
            entry(
                "error",
                Value::Map(
                    [
                        entry("code", Value::Text("invalidValue".into())),
                        entry(
                            "message",
                            Value::Text("a request came before the last one was answered".into()),
                        ),
                    ]
                    .into(),
                ),
            ),
        ]
        .into(),
    ))
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl Core {
    /// A core with no database open.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Core {
        Core {
            bridge: Rc::new(RefCell::new(Some(Bridge::new()))),
        }
    }

    /// Answers `request`: a promise of the answer's bytes, a `Uint8Array`.
    #[cfg(target_arch = "wasm32")]
    pub fn handle(&self, request: Vec<u8>) -> js_sys::Promise {
        let bridge = self.bridge.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let bytes = answer(bridge, request).await;
            Ok(js_sys::Uint8Array::from(bytes.as_slice()).into())
        })
    }
}
