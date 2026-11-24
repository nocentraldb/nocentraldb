# Deploying the relay to AWS

The relay on AWS is the CDK construct `Relay` in `infra/aws/src/relay.ts`, as RFC 0007 describes it ("The AWS backend"). Its function is `crates/ncdb-relay-lambda`, built with its feature `aws`. This guide has two parts:

- **Sections 1 to 4 and "What it costs" are the owner's runbook.** They deploy the project's own relay, the stack `RelayStack` built on the construct, into the account NoCentralDb-Prod, in eu-central-1.
- **"Use the relay in your own stack", "Protect the endpoint" and "What the relay logs" are for app developers.** They put the construct into a stack of their own app and protect its endpoint.

**Deploys run from GitHub Actions** (`.github/workflows/relay-deploy.yml`), as decided on NOC-264. Every merge to `main` that changes the relay's infrastructure or the crates its function is built from deploys it, after you approve the deploy.

- **The account's side lives in the private repository `nocentraldb/aws-deploy`** (Carsten, 2026-11-13): GitHub as an OIDC provider, the deploy role, and CDK's bootstrap with narrowed execution policies. This repository holds no role setup.
- **You approve every change that is deployed.** `.github/CODEOWNERS` makes you the owner of every path the workflow deploys from, and branch protection requires your review (section 1.4). The environment `aws-prod` waits for your approval before each deploy (section 1.3). The agents act on GitHub as an account of their own, so only you can give either approval (section 1.2).
- **No AWS key** is stored anywhere. The deploy job assumes the role with its OIDC token.
- **No agent** ever holds AWS credentials, and no agent or pm merges a change to these paths or approves a deploy.
- **Pull requests** get no AWS access at all. They show their infrastructure change as the diff of the committed template snapshot, `infra/aws/test/__snapshots__/`.

**One stage first: `prod`.** A separate `dev` stage is later work (NOC-264).

## 1. The one-time setup

### 1.1 The account's side, in `nocentraldb/aws-deploy`

Follow the runbook of the private repository `nocentraldb/aws-deploy`. It sets up, in NoCentralDb-Prod and eu-central-1:

- GitHub's OIDC provider;
- the deploy role, which trusts only `repo:nocentraldb/nocentraldb:environment:aws-prod` and may only assume CDK's bootstrap roles;
- CDK's bootstrap, whose CloudFormation execution role is limited to this stack's own resources instead of `AdministratorAccess`.

It ends with the deploy role's ARN, which section 1.3 needs. The agents' account has no access to that repository: its setup is yours. When this repository's stack gains a resource type, the execution policies there must cover it before the change deploys.

### 1.2 The agents' own GitHub account

The factory's agents act on GitHub as a machine account of their own, never as `cabcookie` (Carsten, 2026-11-13). Only you act as `cabcookie`, so GitHub can tell your approval from theirs.

1. **Create a GitHub user** for the agents, with its own email address and two-factor authentication.
2. **Give it Write on `nocentraldb/nocentraldb` only:** the repository's Settings → Collaborators and teams, role "Write". Give it no access to `nocentraldb/aws-deploy`, and no admin or owner role anywhere. With Write it can push branches and open pull requests, but cannot change rulesets, environments or their reviewers.
3. **Connect claude.ai's GitHub as that account:** in claude.ai, disconnect GitHub, sign in to GitHub as the machine account, and connect again. Every Claude session then pushes, opens pull requests and comments as it.
4. **Keep the Claude GitHub App installed** on `nocentraldb/nocentraldb`, so the sessions can reach the repository.
5. **Check:** the next pull request an agent opens shows the machine account as its author, not `cabcookie`.

### 1.3 The GitHub environment `aws-prod`

In the repository's Settings → Environments, create `aws-prod`:

- **Deployment branches and tags:** only `main`.
- **Required reviewers:** `cabcookie` only, never the agents' account. Every deploy then waits in its run until you approve it, after the build and before the job gets its OIDC token. Leave "Prevent self-review" off: you start the deploys yourself by merging.
- **Environment variables.** These are variables, not secrets; none of them is a secret:
  - `AWS_DEPLOY_ROLE_ARN`: the deploy role's ARN from section 1.1.
  - `RELAY_ID`: the relay's stable id, 32 hex digits. Draw it once, with `openssl rand -hex 16`, and never change it: members sign it into every connection's authentication (RFC 0007, "Opening a connection").
  - `ALLOWED_ACCOUNTS`: the identities of the accounts whose members may create spaces, 64 hex digits each, separated by commas.
  - `ALARM_EMAIL`: where the budget and the alarms send their mails.

`ALLOWED_ACCOUNTS` holds each account's identity: its recovery key, which stays the same when the account recovers from its code or replaces a lost device (RFC 0015, "The account's identity"). An account that a database drew before it had a recovery key has no identity but its account key. An app's invitation carries both keys at fixed places. This prints the identity of an invitation `ncdb-invite-1:…`, such as the example app prints with `tasks … invitation`: the recovery key when the invitation carries an account key statement, and the account key when it does not:

```sh
python3 -c 'import base64,sys; t=sys.argv[1].removeprefix("ncdb-invite-1:"); b=base64.urlsafe_b64decode(t+"="*(-len(t)%4)); print((b[163:195] if b[162]==1 else b[65:97]).hex())' "ncdb-invite-1:…"
```

It reads the key without checking the invitation. The local relay does check it: `ncdb-relay --allow-invitation` refuses a malformed one and lists the same identity.

**A list of account keys from before.** Earlier versions of this guide printed the account key of every invitation. Such an entry keeps admitting its account until the account's first recovery or lost device, because the relay also matches an entry against the certificate's account key. Replace each entry with the identity this command prints, before that happens, so the account is not refused with `not_allowed` afterwards.

### 1.4 Branch protection on `main`

In the repository's Settings → Rules → Rulesets, create a branch ruleset for the default branch, with enforcement "Active":

- **Require a pull request before merging**, with:
  - **Require review from Code Owners**;
  - **Dismiss stale pull request approvals when new commits are pushed**.
- **Bypass list:** empty. Above all, never add the agents' account.

With it, a pull request that touches a path in `.github/CODEOWNERS` merges only after you, as `cabcookie`, approved its latest commit, and a push after your approval needs your approval again. The agents' account cannot approve in your place, and cannot change the ruleset. "Required approvals" may stay at 0: the Code Owners rule asks for your approval on the owned paths by itself. Check it once: on the first agent pull request that touches an owned path, GitHub shows "Review required" from a code owner.

**What it costs.** `.github/CODEOWNERS` also owns `Cargo.lock`, `Cargo.toml` and `pnpm-lock.yaml`, because they decide which dependencies the deployed function and the CDK app are built from. So every dependency change in the repository, for any crate or package, needs your review, not only changes to the relay.

## 2. The first deploy

The first deploy runs when a change to the relay reaches `main` after you finished section 1. Before that, the `deploy` job finds no `AWS_DEPLOY_ROLE_ARN` in `aws-prod`, skips every step with a notice and stays green, and nothing is deployed; the merge that brings this runbook is such a run. To deploy once the setup is done, re-run the latest `relay-deploy` run on `main`, or merge the next change to the relay.

1. Watch the workflow `relay-deploy`. Its `deploy` job waits for your approval in the environment `aws-prod`, shows `cdk diff`, then deploys.
2. Confirm the SNS subscription mail that AWS sends to `ALARM_EMAIL`. Until you confirm it, the alarms send nothing.
3. The stack's output `Url` is the relay's URL, `wss://<api id>.execute-api.eu-central-1.amazonaws.com/prod`. It is also in the CloudFormation console, under the stack `NoCentralDbRelay-prod`.

## 3. The smoke test

Run the example app (`apps/tasks-rust/README.md`) against the deployed relay instead of a local one. The laptop's account is the one in `ALLOWED_ACCOUNTS`.

```sh
cargo build -p tasks-rust
alias tasks="$PWD/target/debug/tasks"
RELAY=wss://<api id>.execute-api.eu-central-1.amazonaws.com/prod
LAPTOP=(--data /tmp/laptop --key-file /tmp/laptop.keys --relay "$RELAY")
PHONE=(--data /tmp/phone --key-file /tmp/phone.keys --relay "$RELAY")
```

Then follow steps 4 to 6 of the example's README: create a space, add tasks, invite the phone, sync, watch live updates, complete a task.

Check two more things:

- **A large change.** Add a task whose title is larger than 22 KiB, such as 40 KiB. It goes up through a presigned S3 URL and comes down through another. Both devices show it.
- **Message order.** API Gateway and Lambda do not promise that one connection's messages reach the function in order (#279, "Risk for NOC-272").
  1. Run `tasks "${PHONE[@]}" watch` while you add ten tasks quickly on the laptop.
  2. Every task arrives, and `tasks "${LAPTOP[@]}" sync` ends with nothing unconfirmed.
  3. In CloudWatch Logs, the function's logs hold one JSON line per invocation. During the test, no line may hold the code `unauthenticated` or `malformed`.

If a check fails, roll back (below) and tell pm what you saw, with the log lines. The log lines hold no content.

### 3.1 Capture one real event per route, once (NOC-292)

The function's tests read API Gateway's events from fixtures captured this way during the first smoke test, on 2026-11-17, so the tests read what API Gateway really sends. Capture them again only when API Gateway's events may have changed. The relay itself never logs an event. The capture uses a temporary second function behind a temporary second WebSocket API, and you delete both afterwards.

You need the AWS CLI, signed in to NoCentralDb-Prod with your own admin profile, which may create a Lambda function, an IAM role and an API. The agents' profile `nocentraldb` is read-only and cannot. The commands use your profile and the variable `ACCOUNT`, the account's 12-digit id. They run in bash and in zsh: every variable that a colon follows is written in braces, as in `${ACCOUNT}:role`, because zsh reads `$ACCOUNT:r` as the modifier `:r`.

```sh
export AWS_PROFILE=<your admin profile> AWS_REGION=eu-central-1
aws sso login
ACCOUNT=$(aws sts get-caller-identity --query Account --output text)
```

1. Create the capture function and its API. Its stage is also named `prod`, as the relay's is, because an event carries the stage's name.

   ```sh
   cd "$(mktemp -d)"
   cat > capture.py <<'EOF'
   import json
   def handler(event, context):
       print("NCDB-CAPTURE " + json.dumps(event))
       return {"statusCode": 200}
   EOF
   python3 -m zipfile -c capture.zip capture.py
   aws iam create-role --role-name ncdb-capture --assume-role-policy-document \
     '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}'
   aws iam attach-role-policy --role-name ncdb-capture \
     --policy-arn arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole
   sleep 10   # until IAM hands the new role to Lambda
   FN=$(aws lambda create-function --function-name ncdb-capture --runtime python3.13 \
     --handler capture.handler --zip-file fileb://capture.zip \
     --role "arn:aws:iam::${ACCOUNT}:role/ncdb-capture" --query FunctionArn --output text)
   API=$(aws apigatewayv2 create-api --name ncdb-capture --protocol-type WEBSOCKET \
     --route-selection-expression '$request.body.action' --query ApiId --output text)
   INT=$(aws apigatewayv2 create-integration --api-id "$API" --integration-type AWS_PROXY \
     --integration-uri "arn:aws:apigateway:eu-central-1:lambda:path/2015-03-31/functions/$FN/invocations" \
     --query IntegrationId --output text)
   for route in '$connect' '$disconnect' '$default'; do
     aws apigatewayv2 create-route --api-id "$API" --route-key "$route" --target "integrations/$INT" >/dev/null
   done
   aws apigatewayv2 create-stage --api-id "$API" --stage-name prod --auto-deploy >/dev/null
   aws lambda add-permission --function-name ncdb-capture --statement-id apigw \
     --action lambda:InvokeFunction --principal apigateway.amazonaws.com \
     --source-arn "arn:aws:execute-api:eu-central-1:${ACCOUNT}:${API}/*" >/dev/null
   ```

2. Point the laptop's database from the smoke test at it once. The app opens a connection and sends its `hello`, which holds the protocol's version and no key. The capture function never answers with a challenge, so the app sends nothing more. After 30 seconds `sync` gives up with "not synced after 30s", which is expected here, and the connection closes. That makes one event per route.

   ```sh
   tasks --data /tmp/laptop --key-file /tmp/laptop.keys \
     --relay "wss://$API.execute-api.eu-central-1.amazonaws.com/prod" sync
   sleep 20   # until CloudWatch Logs holds the lines
   ```

3. Turn the three log lines into fixtures. The filter pattern is quoted twice: CloudWatch Logs reads an unquoted term with a hyphen in it as something else and finds no line. The script replaces every id that names your account, the API, the connection, a request or your machine with the fixed values the existing fixtures use. It writes `connect.json`, `message.json` and `disconnect.json`:

   ```sh
   aws logs filter-log-events --log-group-name /aws/lambda/ncdb-capture \
     --filter-pattern '"NCDB-CAPTURE"' --query 'events[].message' --output json > lines.json
   python3 -I - "$ACCOUNT" lines.json <<'EOF'
   import json, re, sys
   account, path = sys.argv[1], sys.argv[2]
   names = {"CONNECT": "connect", "MESSAGE": "message", "DISCONNECT": "disconnect"}
   events = [json.loads(l.split("NCDB-CAPTURE ", 1)[1]) for l in json.load(open(path))]
   first = events[0]["requestContext"]
   fixed = {
       account: "123456789012",
       first["apiId"]: "abcdef1234",
       first["connectionId"]: "Q7mXJdJpFiACFbQ=",
       first["identity"]["sourceIp"]: "192.0.2.10",
   }
   def scrub(value, key=""):
       if isinstance(value, dict):
           return {k: scrub(v, k) for k, v in value.items()}
       if isinstance(value, list):
           return [scrub(v, key) for v in value]
       if not isinstance(value, str):
           return value
       if key in ("requestId", "extendedRequestId", "messageId"):
           return "Q7mXOFpUliAFvdA="
       if key in ("X-Amzn-Trace-Id", "Sec-WebSocket-Key"):
           return {"X-Amzn-Trace-Id": "Root=1-00000000-000000000000000000000000",
                   "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ=="}[key]
       for real, stand_in in fixed.items():
           value = value.replace(real, stand_in)
       return value
   for event in events:
       name = names[event["requestContext"]["eventType"]]
       with open(f"{name}.json", "w") as out:
           json.dump(scrub(event), out, indent=2)
           out.write("\n")
       print(name, "written")
   EOF
   ```

   Read the three files before you hand them on. No address, account id or name of yours may be left; replace by hand what the script missed. The `body` of `message.json` is the app's `hello`. Attach the three files to NOC-292; an agent commits them as the function's test fixtures.

4. Delete the capture function, its API, its role and its logs:

   ```sh
   aws apigatewayv2 delete-api --api-id "$API"
   aws lambda delete-function --function-name ncdb-capture
   aws iam detach-role-policy --role-name ncdb-capture \
     --policy-arn arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole
   aws iam delete-role --role-name ncdb-capture
   aws logs delete-log-group --log-group-name /aws/lambda/ncdb-capture
   ```

## 4. Rollback

- **A bad change.** Revert its commit on `main`. The revert is itself a change to the relay, so the workflow deploys the version before it. The tables and the bucket are kept on every deploy, and on a stack deletion too.
- **Stop all traffic at once.** Throttle the stage to zero, while you look:

  ```sh
  aws apigatewayv2 update-stage --api-id <api id> --stage-name prod \
    --default-route-settings ThrottlingBurstLimit=0,ThrottlingRateLimit=0
  ```

  Members keep working on their own copies and sync when the relay is back (RFC 0007, "Island mode"). To lift it, set the limits back with the same command, for example `ThrottlingBurstLimit=5000,ThrottlingRateLimit=10000`.
- **A failed deploy.** CloudFormation rolls the stack back on its own. The workflow's log says why.
- **Lost or broken data.** Restore the tables with point-in-time recovery, which keeps 35 days. Follow RFC 0007, "Restores and the relay epoch": a restore ends with a new relay epoch before the relay serves anything.

## What it costs, and the alarms

RFC 0007 estimates about 10 USD a month for a small team. The stack holds:

- **A budget of 20 USD a month.** It mails `ALARM_EMAIL` at 50, 80 and 100 percent of actual cost.
- **Two alarms on the function,** one for errors and one for throttles. They mail through SNS.
- **The orphan sweep,** which deletes frames in S3 that no change holds and that are older than a day (RFC 0007, "How large changes go to S3"). It runs every hour at :17, in runs that stop 5 seconds before the function's 29-second timeout and continue where the last stopped, and starts a new pass at most once a day (NOC-293). Each run writes two log lines, `{"sweep":"orphans","phase":"start",…}` and `…"phase":"end",…,"pass":"paused"` (or `"completed"`, `"skipped"`). A failed run counts as an error of the function, so the error alarm mails you. `SweepUnfinished` mails you within the hour when a run started and never ended, and `SweepPassOverdue` when no pass completed in two days.
- **The function's logs,** kept 14 days. They hold ids, sizes, codes and times; no content and no keys.

## Use the relay in your own stack

An app can run its own relay in its own AWS account, with the construct `Relay` in a CDK stack of its own. It is the same relay the project deploys: `RelayStack` is built on it. There is no npm package and no prebuilt function: your CI checks out NoCentralDB at a release tag, builds the function with cargo-lambda, and imports the construct from the checkout.

1. **Check out NoCentralDB at a tag,** for example into `vendor/nocentraldb` in your repository. Do not install the checkout's own dependencies: the construct then uses your app's `aws-cdk-lib`, which must be the version that `infra/aws/package.json` pins at that tag (`aws-cdk-lib` and `constructs`).
2. **Build the function** from the checkout, which needs the Rust toolchain and [cargo-lambda](https://www.cargo-lambda.info/):

   ```sh
   cd vendor/nocentraldb
   cargo lambda build --release --arm64 -p ncdb-relay-lambda --features aws
   # the package is target/lambda/ncdb-relay-lambda, which holds bootstrap
   ```

3. **Put the construct into your stack.** Its files are TypeScript that import each other with the `.ts` extension; run your CDK app with Node 24 (`node bin/app.ts`, as `infra/aws/cdk.json` does) or with a runner such as tsx.

   ```ts
   import { CfnOutput, Stack } from "aws-cdk-lib";
   import { EmailSubscription } from "aws-cdk-lib/aws-sns-subscriptions";
   import { Relay } from "../vendor/nocentraldb/infra/aws/src/relay.ts";

   const relay = new Relay(stack, "Relay", {
     stage: "myapp",
     settings: {
       relayId: "…", // 32 hex digits, drawn once: openssl rand -hex 16
       allowedAccounts: ["…"], // the identities of section 1.3
       admissionKeys: ["…"], // optional: your app's admission key, public half, 64 hex digits
     },
     lambdaPackage: "vendor/nocentraldb/target/lambda/ncdb-relay-lambda",
   });
   relay.addAlarms().addSubscription(new EmailSubscription("ops@example.com"));
   new CfnOutput(stack, "RelayUrl", { value: relay.url });
   ```

What the construct takes and gives:

- **`stage`** names the tables (`ncdb-<stage>-log` and so on), the API and its stage. Table names are fixed, so two relays in one account and region need two stages.
- **`settings`** is what the relay itself is told: `relayId`, which never changes once members use the relay, `allowedAccounts`, as in section 1.3, and `admissionKeys`, below. They become the function's `NCDB_*` variables (`infra/aws/src/relay-settings.ts`), and the construct refuses malformed values when it synthesizes.
- **`admissionKeys`**, optional, lets an app with open signup admit its own users without listing them ([RFC 0015, "Admission tokens"](rfc/0015-relay-admission.md#admission-tokens)). Your app's backend holds an Ed25519 admission key and signs an admission for each user's identity; the relay takes the admissions that verify under one of these public halves, 64 lowercase hex digits each, and sets `NCDB_ADMISSION_KEYS` from them. Left out or empty, the relay takes none. A leaked admission key admits any identity until you remove it; each admitted identity's record names the key that signed it, so you find what it admitted.
- **`addAlarms()`** adds the four alarms of "What it costs" and returns the SNS topic they notify. Without it there are none. The project's budget of 20 USD is part of `RelayStack`, not of the construct; set your own.
- **Public properties:** `api` (the WebSocket API), `stage`, `connectRoute` (the `$connect` route), `handler` (the function), `logGroup` and `url`.

The tables and the bucket are kept when the stack is deleted, as in section 4.

## Protect the endpoint

**Protect your relay's endpoint.** The relay checks every member's signatures and membership, and only the accounts in `allowedAccounts` may create spaces. But anyone who knows the URL can open a connection, and every connection costs you Lambda invocations and DynamoDB requests. The construct leaves the gate in front of it to you, so you can admit connections the way your app signs in its users.

API Gateway's WebSocket APIs take a Lambda authorizer on `$connect`. They have no JWT authorizer; that is only for HTTP APIs. A browser's WebSocket cannot send an `Authorization` header, so the token travels in the relay URL's query string. The SDK opens the URL as you give it, query string included:

```ts
const db = await nc.open({
  model,
  lock,
  relay: `${relayUrl}?token=${encodeURIComponent(accessToken)}`,
});
```

### Example: AWS Cognito

`infra/aws/examples/cognito.ts` is a stack that admits only connections that carry a valid access token of your Cognito user pool; the project's tests synthesize it. It binds a Lambda authorizer to the construct's own `$connect` route with plain CDK:

```ts
import type { CfnRoute } from "aws-cdk-lib/aws-apigatewayv2";
import { WebSocketLambdaAuthorizer } from "aws-cdk-lib/aws-apigatewayv2-authorizers";

const authorizer = new WebSocketLambdaAuthorizer("Cognito", authorize, {
  identitySource: ["route.request.querystring.token"],
});
const { authorizerId, authorizationType } = authorizer.bind({
  route: relay.connectRoute,
  scope: stack,
});
const connect = relay.connectRoute.node.defaultChild as CfnRoute;
connect.authorizationType = authorizationType;
connect.authorizerId = authorizerId;
```

`authorize` is a function of your own, with `USER_POOL_ID` and `USER_POOL_CLIENT_ID` in its environment. API Gateway refuses a connection without `token` before it calls either function. The function checks the token with [aws-jwt-verify](https://github.com/awslabs/aws-jwt-verify) and logs nothing of the request:

```ts
import { CognitoJwtVerifier } from "aws-jwt-verify";

const verifier = CognitoJwtVerifier.create({
  userPoolId: process.env.USER_POOL_ID ?? "",
  clientId: process.env.USER_POOL_CLIENT_ID ?? "",
  tokenUse: "access",
});

export const handler = async (event: {
  methodArn: string;
  queryStringParameters?: Record<string, string>;
}) => {
  let principalId = "anonymous";
  let effect = "Deny";
  try {
    const claims = await verifier.verify(event.queryStringParameters?.token ?? "");
    principalId = claims.sub;
    effect = "Allow";
  } catch {
    // Never log the event or the token: a log is a copy nobody can erase.
  }
  return {
    principalId,
    policyDocument: {
      Version: "2012-10-17",
      Statement: [{ Action: "execute-api:Invoke", Effect: effect, Resource: event.methodArn }],
    },
  };
};
```

The same pattern takes any token your app can check, for example a delegation token an agent got by an RFC 8693 token exchange.

### What it does not do

- **It decides who may connect, not who may create spaces.** The relay still admits only the accounts in `allowedAccounts` to create spaces, and checks every member's signatures and membership as before.
- **It checks the token once, when the connection opens.** A connection that is open when the token expires stays open until it closes; API Gateway closes every connection after at most two hours, and after ten idle minutes.
- **The SDK keeps the URL it was opened with.** It reconnects with the same URL, so once the token has expired, reconnects are refused and the database stops syncing. It keeps working on its own copy and syncs again when your app opens it with a URL that carries a fresh token. The SDK has no option yet to fetch a new token for a reconnect.
- **A token in a URL can land in logs.** API Gateway's WebSocket access logs have no variable for the query string, so they cannot hold it. But anything else that sees the URL can log it: a proxy in front of the relay, your app's own logging, an authorizer that logs its event (AWS's own `$connect` examples log `queryStringParameters`), or your app logging the TypeScript SDK's sync status, which names the relay's URL with its query string. Use short-lived tokens.

## What the relay logs, and what to leave off around it

The relay writes one fixed JSON line per invocation (`crates/ncdb-relay-lambda/src/log.rs`): the route, the connection id, the member id, the message's type, the bytes in and out, error and refusal codes, and when and how long. It never logs an event, a frame, a key or a token, and its logs are kept 14 days. Its tables are keyed by space ids, member ids and change ids, not content, and the changes it stores are ciphertext.

What you add around it can still expose more. Leave these off, or keep them as described:

| Setting | Default | Risk | Rule |
| --- | --- | --- | --- |
| API Gateway execution logging with data trace (`dataTraceEnabled`) | off | logs full message bodies | never on in production |
| API Gateway access logs | off | none for the token: the WebSocket access log has no variable for the query string | allowed, with the `$context` variables you list |
| Lambda logs of your own functions, such as the authorizer | on; CloudWatch keeps them forever by default | anything the code prints, such as the event or its query string | never log the event or the query string; set a retention |
| CloudTrail data events | off | DynamoDB events log full primary keys, S3 events log object keys | never put content into keys |
| X-Ray | off; WebSocket APIs are not traced | only what your code adds | no payloads as metadata |
