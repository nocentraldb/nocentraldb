---
type: fix
tickets: [NOC-585, NOC-572]
introduced: cycle
components: [sdk-rust, sdk-ts]
---
## Symptom
When the app's relay function never settled (its promise or future neither resolved nor rejected), syncing stayed `connecting` for good: no backoff, no retry and no reason in the sync status.

## When
With `relay` given as a function (`relay_with` in Rust) whose call hangs, for example on a token fetch that stalls.

## Cause
The SDK waited for each call of the relay function without a bound.

## Fix
The SDK waits 30 s for each call, natively and in the browser. A call that gives no URL by then ends the attempt as `relay_url_failed`; the SDK retries with the usual backoff and calls the function again, and a late answer of the old call is ignored.
