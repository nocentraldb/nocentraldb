# Deploying the relay to AWS

This runbook is for the owner. It deploys the relay as RFC 0007 describes it ("The AWS backend") into the account NoCentralDb-Prod, in eu-central-1. The infrastructure is the CDK app in `infra/aws`. The function is `crates/ncdb-relay-lambda`, built with its feature `aws`.

**Deploys run from GitHub Actions** (`.github/workflows/relay-deploy.yml`), as decided on NOC-264. You set up the trust between GitHub and the account once. From then on, every merge to `main` that changes the relay's infrastructure or the crates its function is built from deploys it.

- **No AWS key** is stored anywhere. The deploy job assumes a role with its OIDC token.
- **No agent** ever holds AWS credentials.
- **Pull requests** get no AWS access at all. They show their infrastructure change as the diff of the committed template snapshot, `infra/aws/test/__snapshots__/`.

**One stage first: `prod`.** A separate `dev` stage is later work (NOC-264).

## 1. The one-time trust setup

You need the AWS CLI, signed in to NoCentralDb-Prod with rights to create IAM roles. The commands use the profile `nocentraldb` and the variable `ACCOUNT`, the account's 12-digit id:

```sh
export AWS_PROFILE=nocentraldb AWS_REGION=eu-central-1
aws sso login --use-device-code --no-browser
ACCOUNT=$(aws sts get-caller-identity --query Account --output text)
```

### 1.1 GitHub as an identity provider

```sh
aws iam create-open-id-connect-provider \
  --url https://token.actions.githubusercontent.com \
  --client-id-list sts.amazonaws.com
```

### 1.2 CDK's bootstrap

Run this once from the repository, after `pnpm install`:

```sh
cd infra/aws
pnpm exec cdk bootstrap "aws://$ACCOUNT/eu-central-1"
```

The bootstrap creates CDK's own roles (`cdk-hnb659fds-*`) and its asset bucket. CloudFormation deploys the stack through CDK's execution role. By default that role holds `AdministratorAccess`, and only CloudFormation can assume it. To narrow it, pass `--cloudformation-execution-policies` with a policy of your own.

### 1.3 The deploy role

The deploy role trusts only GitHub's OIDC tokens for this repository's environment `aws-prod`. It can do one thing: assume CDK's bootstrap roles. It gets no direct rights on any resource.

```sh
cat > trust.json <<EOF
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": { "Federated": "arn:aws:iam::$ACCOUNT:oidc-provider/token.actions.githubusercontent.com" },
    "Action": "sts:AssumeRoleWithWebIdentity",
    "Condition": {
      "StringEquals": {
        "token.actions.githubusercontent.com:aud": "sts.amazonaws.com",
        "token.actions.githubusercontent.com:sub": "repo:nocentraldb/nocentraldb:environment:aws-prod"
      }
    }
  }]
}
EOF
cat > assume-cdk.json <<EOF
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": "sts:AssumeRole",
    "Resource": "arn:aws:iam::$ACCOUNT:role/cdk-hnb659fds-*-$ACCOUNT-eu-central-1"
  }]
}
EOF
aws iam create-role --role-name ncdb-relay-deploy --assume-role-policy-document file://trust.json
aws iam put-role-policy --role-name ncdb-relay-deploy --policy-name assume-cdk-roles \
  --policy-document file://assume-cdk.json
rm trust.json assume-cdk.json
```

### 1.4 The GitHub environment `aws-prod`

In the repository's Settings → Environments, create `aws-prod`:

- **Deployment branches and tags:** only `main`.
- **Environment variables.** These are variables, not secrets; none of them is a secret:
  - `AWS_DEPLOY_ROLE_ARN`: `arn:aws:iam::<ACCOUNT>:role/ncdb-relay-deploy`.
  - `RELAY_ID`: the relay's stable id, 32 hex digits. Draw it once, with `openssl rand -hex 16`, and never change it: members sign it into every connection's authentication (RFC 0007, "Opening a connection").
  - `ALLOWED_ACCOUNTS`: the account keys whose members may create spaces, 64 hex digits each, separated by commas.
  - `ALARM_EMAIL`: where the budget and the alarms send their mails.

`ALLOWED_ACCOUNTS` holds an account key, which an app's invitation carries at a fixed place. This prints the account key of an invitation `ncdb-invite-1:…`, such as the example app prints with `tasks … invitation`:

```sh
python3 -c 'import base64,sys; t=sys.argv[1].removeprefix("ncdb-invite-1:"); print(base64.urlsafe_b64decode(t+"="*(-len(t)%4))[65:97].hex())' "ncdb-invite-1:…"
```

It reads the key without checking the invitation. The local relay does check it: `ncdb-relay --allow-invitation` refuses a malformed one.

## 2. The first deploy

The first deploy runs when a change to the relay reaches `main`, for example the merge that brings this runbook.

1. Watch the workflow `relay-deploy`. Its `deploy` job waits for the environment `aws-prod`, shows `cdk diff`, then deploys.
2. Confirm the SNS subscription mail that AWS sends to `ALARM_EMAIL`. Until you confirm it, the alarms send nothing.
3. The stack's output `Url` is the relay's URL, `wss://<api id>.execute-api.eu-central-1.amazonaws.com/prod`. It is also in the CloudFormation console, under the stack `NoCentralDbRelay-prod`.

## 3. The smoke test

Run the example app (`apps/tasks-rust/README.md`) against the deployed relay instead of a local one. The laptop's account is the one in `ALLOWED_ACCOUNTS`.

```sh
cargo build -p tasks-rust
alias tasks=./target/debug/tasks
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
