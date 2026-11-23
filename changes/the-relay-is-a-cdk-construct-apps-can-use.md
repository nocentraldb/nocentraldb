---
type: feature
tickets: [NOC-417]
components: [relay]
---
## Problem
An app that runs its own relay on AWS could only deploy the project's whole stack, with the project's budget and alarms, and had no way to put its own gate in front of the relay's endpoint.

## Use
Put the `Relay` construct from `infra/aws/src/relay.ts` into a CDK stack of your own, and attach your own authorizer to its `$connect` route, for example one that checks a Cognito access token in the relay URL's query string. docs/relay-deploy.md, "Use the relay in your own stack" and "Protect the endpoint", shows how.
