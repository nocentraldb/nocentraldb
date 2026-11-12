//! The clock and the timers of syncing (NOC-175): what a session waits
//! for, its deadlines and the backoff between attempts. Natively they are
//! tokio's and `std::time::Instant`, as before. On wasm32, where
//! `std::time::Instant` panics and tokio has no timers, they are the
//! global's `setTimeout` and `performance.now()`, in a window, a worker or
//! Node, through `js_sys`.

#[cfg(not(target_arch = "wasm32"))]
use std::future::Future;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

/// A wait that did not end in time.
#[derive(Debug)]
pub(crate) struct Elapsed;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::time::Instant;

/// Waits for `duration`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

/// The output of `future`, or [`Elapsed`] when it does not end within
/// `duration`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn timeout<F: Future>(
    duration: Duration,
    future: F,
) -> Result<F::Output, Elapsed> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| Elapsed)
}

/// Waits until `deadline`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn sleep_until(deadline: Instant) {
    tokio::time::sleep_until(deadline.into()).await;
}

// Only the tests use them on wasm32 until the browser's session does.
#[cfg(target_arch = "wasm32")]
#[allow(unused_imports)]
pub(crate) use browser::{Instant, sleep, sleep_until, timeout};

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::future::Future;
    use std::ops::Add;
    use std::pin::pin;
    use std::task::Poll;
    use std::time::Duration;

    use js_sys::{Function, Promise, Reflect};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    use super::Elapsed;

    /// A moment on the global's monotonic clock, `performance.now()`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct Instant(Duration);

    impl Instant {
        /// Now.
        pub(crate) fn now() -> Instant {
            Instant(Duration::from_secs_f64(now_millis().max(0.0) / 1000.0))
        }

        /// How long ago this was.
        pub(crate) fn elapsed(&self) -> Duration {
            Instant::now().saturating_duration_since(*self)
        }

        /// How long after `earlier` this is, or zero when it is before it.
        pub(crate) fn saturating_duration_since(&self, earlier: Instant) -> Duration {
            self.0.saturating_sub(earlier.0)
        }
    }

    impl Add<Duration> for Instant {
        type Output = Instant;

        fn add(self, duration: Duration) -> Instant {
            Instant(self.0 + duration)
        }
    }

    /// `performance.now()` in milliseconds, or `Date.now()` where the
    /// global has no `performance`.
    fn now_millis() -> f64 {
        let global = js_sys::global();
        let performance = Reflect::get(&global, &JsValue::from_str("performance")).ok();
        let now = performance.as_ref().and_then(|performance| {
            let now = Reflect::get(performance, &JsValue::from_str("now")).ok()?;
            now.dyn_into::<Function>()
                .ok()?
                .call0(performance)
                .ok()?
                .as_f64()
        });
        now.unwrap_or_else(js_sys::Date::now)
    }

    /// Waits for at least `duration` by [`Instant`].
    pub(crate) async fn sleep(duration: Duration) {
        sleep_until(Instant::now() + duration).await;
    }

    /// Waits until `deadline` has passed by [`Instant`]. A timer of the
    /// global's `setTimeout` may fire a little before its delay by
    /// `performance.now()`, as Node's does, so the wait goes on until the
    /// deadline has passed, with each timer rounded up to whole
    /// milliseconds.
    pub(crate) async fn sleep_until(deadline: Instant) {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            timer(left).await;
        }
    }

    /// One timer of the global's `setTimeout` for `duration`, rounded up to
    /// whole milliseconds.
    async fn timer(duration: Duration) {
        let millis = duration.as_micros().div_ceil(1000).min(i32::MAX as u128) as f64;
        let promise = Promise::new(&mut |resolve, _| {
            let global = js_sys::global();
            let set_timeout = Reflect::get(&global, &JsValue::from_str("setTimeout"))
                .ok()
                .and_then(|f| f.dyn_into::<Function>().ok());
            match set_timeout {
                Some(set_timeout) => {
                    let _ = set_timeout.call2(&global, &resolve, &JsValue::from_f64(millis));
                }
                None => {
                    let _ = resolve.call0(&JsValue::UNDEFINED);
                }
            }
        });
        let _ = JsFuture::from(promise).await;
    }

    /// The output of `future`, or [`Elapsed`] when it does not end within
    /// `duration`.
    pub(crate) async fn timeout<F: Future>(
        duration: Duration,
        future: F,
    ) -> Result<F::Output, Elapsed> {
        let mut future = pin!(future);
        let mut timer = pin!(sleep(duration));
        std::future::poll_fn(|cx| {
            if let Poll::Ready(output) = future.as_mut().poll(cx) {
                return Poll::Ready(Ok(output));
            }
            match timer.as_mut().poll(cx) {
                Poll::Ready(()) => Poll::Ready(Err(Elapsed)),
                Poll::Pending => Poll::Pending,
            }
        })
        .await
    }
}
