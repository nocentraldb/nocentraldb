# 2026-11-23 · NOC-417 · The relay on AWS ships as a CDK construct apps use, and each app protects its endpoint itself

- Decided by: Carsten (2026-11-23, on NOC-417)
- Why: an app such as nexflow.it wraps the relay in its own stack and admits connections the way it signs in its users, for example with Cognito; a reusable `Relay` construct in `infra/aws`, with `RelayStack` built on it, gives it that with plain CDK.
  - The construct exposes its WebSocket API and `$connect` route, so the app attaches its own authorizer; there is no authorizer prop and no new SDK option. The token travels in the relay URL's query string, which the SDK passes unchanged.
  - docs/relay-deploy.md says plainly that the app should protect the endpoint, shows how with Cognito, and names the trade-offs: a token in a URL can land in logs, so use short-lived tokens; admission still decides who may create spaces.
  - Apps still build the function with cargo-lambda.
- Instead of: a prebuilt Lambda as a release asset and an npm package for the construct (option B); an authorizer prop on the construct; an SDK option that sends the token as a WebSocket subprotocol.
- Topics: relay, aws, cdk, security
