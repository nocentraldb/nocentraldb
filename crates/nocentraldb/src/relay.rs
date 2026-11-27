//! The relay a database syncs with, as the app gave it (NOC-496): a fixed
//! URL, from [`OpenOptions::relay`](crate::OpenOptions::relay), or the
//! app's function that gives the URL, from
//! [`OpenOptions::relay_with`](crate::OpenOptions::relay_with), which the
//! sync calls before each connect, so a short-lived token in the URL is
//! renewed. The sync waits 30 s for each call of the function; one that
//! gives no URL by then counts as failed, and its late answer is ignored
//! (Carsten, 2026-11-27, NOC-572).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

/// How long the sync waits for each call of the app's function.
pub(crate) const BOUND: Duration = Duration::from_secs(30);

/// The app's function, boxed: its future gives the URL, or none when the
/// function failed. Its future need not be `Send`: the sync runs on a
/// thread of its own, or on the browser's event loop.
pub(crate) type Given =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<String>>>> + Send + Sync>;

/// The relay of a database's options.
#[derive(Clone)]
pub(crate) enum Relay {
    Fixed(String),
    /// The app's function, and how long the sync waits for each call.
    Given(Given, Duration),
}

impl Relay {
    /// The relay the app's function `given` gives, waited for [`BOUND`].
    pub(crate) fn given(given: Given) -> Relay {
        Relay::Given(given, bound())
    }

    /// The URL to connect with now: the fixed one, or what the app's
    /// function gives, when it is a relay URL. `None` when the function
    /// failed, gave no relay URL or gave none within its bound; a call
    /// that ran out of time is dropped, so its late answer reaches no one.
    /// What the function failed with is dropped, never logged, since it
    /// comes from the app and may carry anything.
    pub(crate) async fn url(&self) -> Option<String> {
        match self {
            Relay::Fixed(url) => Some(url.clone()),
            Relay::Given(given, bound) => crate::sync::time::timeout(*bound, given())
                .await
                .ok()
                .flatten()
                .filter(|url| is_relay_url(url)),
        }
    }
}

/// How long the sync waits for a function given now: [`BOUND`], or what a
/// test set on this thread.
fn bound() -> Duration {
    #[cfg(test)]
    if let Some(bound) = TEST_BOUND.get() {
        return bound;
    }
    BOUND
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
    static TEST_BOUND: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

/// Runs `f` with `bound` in place of the 30 s for each relay function
/// given meanwhile on this thread: only the tests shorten it.
#[cfg(test)]
pub(crate) fn with_bound<T>(bound: Duration, f: impl FnOnce() -> T) -> T {
    let before = TEST_BOUND.replace(Some(bound));
    let output = f();
    TEST_BOUND.set(before);
    output
}
