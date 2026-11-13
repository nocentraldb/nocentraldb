# 2026-11-12 · NOC-272 · Large changes on AWS go through presigned S3 URLs, and the backend takes an upload whose object holds its declared size

- Decided by: coding agent (NOC-272), with pm's answer A, following RFC 0007, "How large changes go to S3"
- Why: behind API Gateway, no listener serves the relay's own upload and download URLs. So the handler's `Urls` gets the upload (id, token, size) and the entry rather than a token, and `S3Urls` presigns:
  - a PUT to `uploads/<upload id>`, valid 15 minutes, with the size fixed in the signature;
  - a GET from `frames/<space id>/<change id>`, valid 5 minutes.

  The relay core's `page::read` asks a `Downloads` for an entry's URL. `TokenDownloads` keeps the WebSocket server's and the harness's token URLs as before.

  A frame put through a presigned URL is never marked filled, since no `fill_upload` runs. So the AWS backend's `take_upload` also takes an upload whose S3 object holds exactly its declared size, and it still takes it once, by the conditional delete of its item.

  Presigning leaves the backend's `tokens` table unused behind API Gateway. pm takes its removal to the owner as its own question; this change does not delete it.
- Instead of: serving the token URLs through a second, HTTP API in front of the same function, which RFC 0007 does not describe and which would pass large frames through Lambda; or marking an upload filled through an S3 event, which adds a trigger and a second function.
- Topics: relay, aws, s3, large-changes
