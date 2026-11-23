//! The app's admission function (RFC 0015, "In the SDK"): what
//! [`OpenOptions::admission`](crate::OpenOptions::admission) keeps, and the
//! account's identity it is called with. The sync calls it when the relay
//! refuses a genesis with `not_allowed` (`sync::session::admission`).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::db::Inner;
use crate::keys;

/// The app's function, boxed: its future gives the admission's text, or
/// none when the app's backend failed. Its future need not be `Send`: the
/// sync runs on a thread of its own, or on the browser's event loop.
pub(crate) type Fetch =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<String>>>> + Send + Sync>;

/// `fetch` boxed as a [`Fetch`]: its error is dropped, never logged, since
/// it comes from the app's backend and may carry anything.
pub(crate) fn boxed<F, Fut, E>(fetch: F) -> Fetch
where
    F: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, E>> + 'static,
{
    Arc::new(move |identity| {
        let future = fetch(identity);
        Box::pin(async move { future.await.ok() })
    })
}

/// The account's identity, its recovery key in lowercase hexadecimal, or
/// none while the database waits to be paired (RFC 0015, "The account's
/// identity").
pub(crate) fn identity(inner: &Inner) -> Option<String> {
    let statement = match &inner.account {
        keys::Account::Holds { statement, .. } | keys::Account::Agent { statement, .. } => {
            statement
        }
        keys::Account::None => return None,
    };
    Some(
        statement
            .recovery_key
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}
