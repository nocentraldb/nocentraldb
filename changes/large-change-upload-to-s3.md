---
type: fix
tickets: [NOC-348, NOC-349]
introduced: cycle
components: [sdk-rust, sdk-ts]
---
## Symptom

A large change, one above 22 KiB such as a task with a 30 KB title, never reached a relay on AWS. `tasks sync` ended with "not synced after 60s", the status went from Connecting to Offline and back, and the relay handed out a new upload URL every few seconds.

## When

On a relay whose upload URLs are presigned S3 URLs, as the AWS relay's are, from the Rust SDK and the command line. The browser was not affected: it uploads with `fetch()`.

## Cause

The SDK's HTTP client sent `Host: <bucket host>:443` for an `https://` URL without a port. A presigned S3 URL signs the `host` header as the URL names it, without the port, so S3 refused every upload. The SDK then dropped the connection, reconnected and asked for a new upload URL, without end and without saying why.

## Fix

The `Host` header names the URL's authority exactly as the URL writes it. An upload that the URL refuses with an HTTP status no longer drops the connection: `SyncStatus.error` shows the new code `upload_failed`, the change stays unconfirmed, and the SDK uploads it again after a backoff, which starts at 100 ms and doubles up to 2 minutes. An upload that goes through clears the code.
