//! The clock and the timers of syncing (NOC-175): natively tokio's, and in
//! the browser the global's `setTimeout` and `performance.now()`, so the
//! session's waits and deadlines run on wasm32 as they do natively. The
//! tests run on both targets: natively on tokio, on wasm32 in Node.

use std::future::pending;
use std::time::Duration;

use super::time::{Instant, sleep, timeout};

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn sleep_waits_at_least_its_duration() {
    let start = Instant::now();

    sleep(Duration::from_millis(30)).await;

    assert!(
        start.elapsed() >= Duration::from_millis(30),
        "{:?}",
        start.elapsed()
    );
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_future_that_ends_in_time_gives_its_output() {
    let answer = timeout(Duration::from_secs(5), async { 42 }).await;

    assert_eq!(answer.ok(), Some(42));
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn a_future_that_does_not_end_in_time_times_out_after_its_duration() {
    let start = Instant::now();

    let answer = timeout(Duration::from_millis(30), pending::<()>()).await;

    assert!(answer.is_err());
    assert!(
        start.elapsed() >= Duration::from_millis(30),
        "{:?}",
        start.elapsed()
    );
}

#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
async fn instants_are_ordered_and_add_durations() {
    let earlier = Instant::now();
    sleep(Duration::from_millis(5)).await;
    let later = Instant::now();

    assert!(later > earlier);
    assert!(earlier + Duration::from_secs(1) > later);
    assert_eq!(
        (earlier + Duration::from_secs(1)).saturating_duration_since(earlier),
        Duration::from_secs(1)
    );
    assert_eq!(earlier.saturating_duration_since(later), Duration::ZERO);
}
