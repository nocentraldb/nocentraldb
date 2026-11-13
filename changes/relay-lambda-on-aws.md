---
type: internal
tickets: [NOC-272]
components: [relay]
---
The relay's Lambda function runs on the AWS backend: a `connections` table on DynamoDB, API Gateway's connection API and presigned S3 URLs for large changes, with settings from its environment.
