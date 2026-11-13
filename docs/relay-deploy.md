# Deploying the relay to AWS

This runbook is for the owner. It deploys the relay as RFC 0007 describes it ("The AWS backend") into the account NoCentralDb-Prod, in eu-central-1. The infrastructure is the CDK app in `infra/aws`. The function is `crates/ncdb-relay-lambda`, built with its feature `aws`.

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
  - `ALLOWED_ACCOUNTS`: the account keys whose members may create spaces, 64 hex digits each, separated by commas.
  - `ALARM_EMAIL`: where the budget and the alarms send their mails.

`ALLOWED_ACCOUNTS` holds an account key, which an app's invitation carries at a fixed place. This prints the account key of an invitation `ncdb-invite-1:…`, such as the example app prints with `tasks … invitation`:

```sh
python3 -c 'import base64,sys; t=sys.argv[1].removeprefix("ncdb-invite-1:"); print(base64.urlsafe_b64decode(t+"="*(-len(t)%4))[65:97].hex())' "ncdb-invite-1:…"
```

It reads the key without checking the invitation. The local relay does check it: `ncdb-relay --allow-invitation` refuses a malformed one.

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
LAPTOP="--data /tmp/laptop --key-file /tmp/laptop.keys --relay $RELAY"
PHONE="--data /tmp/phone --key-file /tmp/phone.keys --relay $RELAY"
```

Then follow steps 4 to 6 of the example's README: create a space, add tasks, invite the phone, sync, watch live updates, complete a task.

Check two more things:

- **A large change.** Add a task whose title is larger than 128 KiB. It goes up through a presigned S3 URL and comes down through another. Both devices show it.
- **Message order.** API Gateway and Lambda do not promise that one connection's messages reach the function in order (#279, "Risk for NOC-272").
  1. Run `tasks $PHONE watch` while you add ten tasks quickly on the laptop.
  2. Every task arrives, and `tasks $LAPTOP sync` ends with nothing unconfirmed.
  3. In CloudWatch Logs, the function's logs hold one JSON line per invocation. During the test, no line may hold the code `unauthenticated` or `malformed`.

If a check fails, roll back (below) and tell pm what you saw, with the log lines. The log lines hold no content.

### 3.1 Capture one real event per route, once (NOC-292)

The function's tests read API Gateway's events from fixtures that were written by hand. Capture one real event per route during this first smoke test, so the tests read what API Gateway really sends. The relay itself never logs an event. The capture uses a temporary second function behind a temporary second WebSocket API, and you delete both afterwards.

You need the AWS CLI, signed in to NoCentralDb-Prod with rights to create a Lambda function, an IAM role and an API. The commands use the profile `nocentraldb` and the variable `ACCOUNT`, the account's 12-digit id:

```sh
export AWS_PROFILE=nocentraldb AWS_REGION=eu-central-1
aws sso login --use-device-code --no-browser
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
     --role "arn:aws:iam::$ACCOUNT:role/ncdb-capture" --query FunctionArn --output text)
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
     --source-arn "arn:aws:execute-api:eu-central-1:$ACCOUNT:$API/*" >/dev/null
   ```

2. Point the laptop's database from the smoke test at it once. The app opens a connection and sends its `hello`, which holds the protocol's version and no key. The capture function never answers with a challenge, so the app sends nothing more, `timeout` ends it, and the connection closes. That makes one event per route.

   ```sh
   timeout 15 tasks --data /tmp/laptop --key-file /tmp/laptop.keys \
     --relay "wss://$API.execute-api.eu-central-1.amazonaws.com/prod" sync
   sleep 20   # until CloudWatch Logs holds the lines
   ```

3. Turn the three log lines into fixtures. The script replaces every id that names your account, the API, the connection, a request or your machine with the fixed values the existing fixtures use. It writes `connect.json`, `message.json` and `disconnect.json`:

   ```sh
   aws logs filter-log-events --log-group-name /aws/lambda/ncdb-capture \
     --filter-pattern NCDB-CAPTURE --query 'events[].message' --output json > lines.json
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
- **The function's logs,** kept 14 days. They hold ids, sizes, codes and times; no content and no keys.
