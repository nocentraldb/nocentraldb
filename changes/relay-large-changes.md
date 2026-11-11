---
type: feature
tickets: [NOC-102]
components: [relay]
---
## Problem
The relay server refused every change whose frame did not fit into one 128 KiB message with `too_large`, such as a large paste into a rich-text field.

## Use
A member asks `upload {space, size}` and gets an upload id and a one-time URL. It sends the frame with `PUT <url>` within 15 minutes, with the declared size as `Content-Length`, and pushes `{1: upload id}`. The relay checks that frame like any other, up to 16 MiB. On a pull, an entry whose frame does not fit inline carries `{1: url, 2: size}`, and `GET <url>` serves the frame for 5 minutes. The same listener serves both URLs. Behind a TLS proxy, start the server with `--public-url https://relay.example.com`.
