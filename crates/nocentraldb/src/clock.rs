//! The device's wall clock, in milliseconds since the Unix epoch, on every
//! target: the system clock natively, and JavaScript's `Date.now()` on
//! wasm32, where `std::time::SystemTime::now` panics (NOC-175). A clock
//! before the epoch reads as 0.

/// The wall clock now, in milliseconds since the Unix epoch.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The wall clock now, in milliseconds since the Unix epoch.
#[cfg(target_arch = "wasm32")]
pub(crate) fn now_millis() -> u64 {
    // Date.now() is a whole number of milliseconds, at most 8.64e15.
    let now = js_sys::Date::now();
    if now.is_finite() && now > 0.0 {
        now as u64
    } else {
        0
    }
}
