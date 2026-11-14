# Testing the relay's AWS backend against the AWS Emulator

The relay's AWS backend (RFC 0007, "The AWS backend") runs on DynamoDB, S3, API Gateway and Lambda. Its tests run against the [AWS Emulator](https://docs.aws.amazon.com/emulator/latest/userguide/what-is-service.html), which serves the AWS APIs on this machine with no AWS account (NOC-264). The emulator does not match AWS everywhere. The coverage check in `crates/ncdb-relay-aws/tests/emulator_coverage.rs` says, per operation the backend needs, whether the emulator supports it, behaves differently or answers `NotImplemented`.

The CI job `.github/workflows/aws-emulator.yml` runs the same steps on one pinned emulator version.

## Install the emulator

On Linux (x86_64), install the signed Debian package, after checking its signature as the emulator's [Getting started on Linux](https://docs.aws.amazon.com/emulator/latest/userguide/getting-started-linux.html) says. A specific version's file is `aws-emulator_<MAJOR.MINOR.PATCH>-1_amd64.deb`, for example `aws-emulator_1.0.336-1_amd64.deb`. `versions.json` lists versions with a fourth part, `1.0.336.1`, which the file name does not carry. On macOS and Windows, run it in Docker, as the emulator's docs say.

Embedded mode, the emulator compiled into the tests as a dev-dependency, is not used: it is a git dependency, which `scripts/check-pins.mjs` refuses, and its build needs about 100 GB of disk.

## Run it with telemetry off

The emulator reports usage data to AWS by default. Every run of ours turns that off:

```sh
export EMULATOR_TELEMETRY=off
aws-emulator --telemetry off
```

It serves HTTPS on `127.0.0.1:9297` and writes its credentials, an SDK config file and its certificate to `~/.aws/emulator/9297/`. They work only for this run, and the emulator deletes them when it stops.

## Point the tests at it

In the shell that runs the tests, with no real AWS credentials in it:

```sh
unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN AWS_PROFILE
export AWS_ENDPOINT_URL=https://127.0.0.1:9297
export AWS_SHARED_CREDENTIALS_FILE=~/.aws/emulator/9297/emulator_credentials
export AWS_CONFIG_FILE=~/.aws/emulator/9297/config
export SSL_CERT_FILE=~/.aws/emulator/9297/ca.pem   # the AWS SDK for Rust trusts the emulator's CA through this
export AWS_EC2_METADATA_DISABLED=true
export AWS_DISABLE_HOST_PREFIX_INJECTION=true      # a host prefix such as data-127.0.0.1 would miss the emulator
export AWS_REGION=us-east-1                        # the emulator holds one region
export AWS_DEFAULT_REGION=us-east-1
cargo test -p ncdb-relay-aws --test emulator_coverage -- --ignored --nocapture
```

- **The CA.** The emulator makes a new certificate each time it starts; `SSL_CERT_FILE` points at the file, so it keeps working.
- **Path-style S3.** The endpoint is an IP address, so S3 requests must carry the bucket in the path. The tests build their S3 client with `force_path_style(true)`.
- **Host-prefix injection off.** `AWS_DISABLE_HOST_PREFIX_INJECTION` keeps the SDK from sending a request to a host name the emulator does not serve.

With the same variables set, the AWS backend's tests run against the emulator: RFC 0007's conformance suite, and the backend's own tests of where a frame is kept, orphans in S3, concurrent appends, the epoch item and the size of one transaction. Each test makes its own tables and bucket under a random prefix and deletes them at its end.

```sh
cargo test -p ncdb-relay-aws --test conformance --test aws_backend -- --ignored
```

The coverage check and the backend's tests refuse to run when `AWS_ENDPOINT_URL` is not a loopback address or when real credentials are set, so they never reach AWS. The coverage check prints a table of results, and writes it to the file `NCDB_EMULATOR_REPORT` names, when it is set.

## Known gaps

- **API Gateway's WebSocket data plane (NOC-278).** Emulator 1.0.336 refuses the WebSocket upgrade on a stage, so it never issues a connection id. It answers `@connections` for an id it never issued with 400 `BadRequestException`, where AWS answers 410 `GoneException`. The relay's delivery to a gone subscriber relies on the 410, so it cannot be tested against the emulator yet.
  - `delivery_to_a_gone_subscriber_waits_for_noc_278` in `crates/ncdb-relay-lambda/tests/emulator.rs` pins the gap. It fails the day the emulator answers 410; that day it becomes the delivery test.
  - Until then, two tests stand in for it, each marked as doing so: a local endpoint that answers as AWS does (`tests/apigw.rs`), and the handler forgetting a gone connection (`tests/handler.rs`).
  - The coverage check's row "WebSocket upgrade on the stage" shows when the emulator starts to serve connections.

## What the emulator does not replace

It does not evaluate IAM as AWS does, it holds one region, and it keeps its state in memory. A smoke test on a real `dev` stage before `prod` stays (NOC-264).
