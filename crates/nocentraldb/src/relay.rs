//! The relay a database syncs with, as the app gave it (NOC-496): a fixed
//! URL, from [`OpenOptions::relay`](crate::OpenOptions::relay), or the
//! app's function that gives the URL, from
//! [`OpenOptions::relay_with`](crate::OpenOptions::relay_with), which the
//! sync calls before each connect, so a short-lived token in the URL is
//! renewed.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// The app's function, boxed: its future gives the URL, or none when the
/// function failed. Its future need not be `Send`: the sync runs on a
/// thread of its own, or on the browser's event loop.
pub(crate) type Given =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<String>>>> + Send + Sync>;

/// The relay of a database's options.
#[derive(Clone)]
pub(crate) enum Relay {
    Fixed(String),
    Given(Given),
}

impl Relay {
    /// The URL to connect with now: the fixed one, or what the app's
    /// function gives, when it is a relay URL. `None` when the function
    /// failed or gave no relay URL; what it failed with is dropped, never
    /// logged, since it comes from the app and may carry anything.
    pub(crate) async fn url(&self) -> Option<String> {
        match self {
            Relay::Fixed(url) => Some(url.clone()),
            Relay::Given(given) => given().await.filter(|url| is_relay_url(url)),
        }
    }
}

/// Whether `url` is a relay URL: `wss://`, or `ws://` on a local network.
pub(crate) fn is_relay_url(url: &str) -> bool {
    url.starts_with("wss://") || url.starts_with("ws://")
}

/// `given` boxed as a [`Given`].
pub(crate) fn boxed<F, Fut, E>(given: F) -> Given
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, E>> + 'static,
{
    Arc::new(move || {
        let future = given();
        Box::pin(async move { future.await.ok() })
    })
}

#[cfg(test)]
thread_local! {
    /// The bound a test sets for the relay functions given on its thread.
    static TEST_BOUND: std::cell::Cell<Option<std::time::Duration>> =
        const { std::cell::Cell::new(None) };
}

/// Runs `f` with `bound` in place of the 30 s for each relay function
/// given meanwhile on this thread: only the tests shorten it.
#[cfg(test)]
pub(crate) fn with_bound<T>(bound: std::time::Duration, f: impl FnOnce() -> T) -> T {
    let before = TEST_BOUND.replace(Some(bound));
    let output = f();
    TEST_BOUND.set(before);
    output
}
