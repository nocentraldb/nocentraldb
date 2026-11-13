---
type: internal
tickets: [NOC-272]
components: [relay]
---
The relay's AWS infrastructure is one CDK stack in `infra/aws`: the WebSocket API, the function, the eight tables, the frames bucket, logs, alarms and a 20 USD budget. Its tests synthesize it with no AWS credentials.
