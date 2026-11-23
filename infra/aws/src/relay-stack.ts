// The project's own relay on AWS, in eu-central-1, in the account
// NoCentralDb-Prod: the `Relay` construct (./relay.ts) with its alarms, a
// budget of 20 USD a month with mails at 50, 80 and 100 percent (NOC-264
// point 5), and the relay's URL as an output.
//
// The construct's id is `Default`, which CDK leaves out of logical ids, so
// every resource keeps the logical id it had when the stack held them
// directly (NOC-417): a deploy replaces or renames nothing.

import { CfnOutput, Stack, type StackProps } from "aws-cdk-lib";
import { CfnBudget } from "aws-cdk-lib/aws-budgets";
import { EmailSubscription } from "aws-cdk-lib/aws-sns-subscriptions";
import type { Construct } from "constructs";
import { Relay } from "./relay.ts";

/// What the project's deployment of the relay sets.
export interface RelayStackProps extends StackProps {
  /// The stage, which names the tables and the API stage: `prod`.
  readonly stage: string;
  /// The relay's stable id, 32 hex digits, drawn once per deployment.
  readonly relayId: string;
  /// The identities of the accounts whose members may create spaces, 64
  /// hex digits each: each account's recovery key, or the account key of
  /// an account without one (RFC 0015, "The account's identity").
  readonly allowedAccounts: readonly string[];
  /// Where the budget and the alarms send their mails.
  readonly alarmEmail: string;
  /// The directory of the function's built package, which holds `bootstrap`.
  readonly lambdaPackage: string;
}

export class RelayStack extends Stack {
  constructor(scope: Construct, id: string, props: RelayStackProps) {
    super(scope, id, props);
    const relay = new Relay(this, "Default", {
      stage: props.stage,
      settings: {
        relayId: props.relayId,
        allowedAccounts: props.allowedAccounts,
      },
      lambdaPackage: props.lambdaPackage,
    });
    relay.addAlarms().addSubscription(new EmailSubscription(props.alarmEmail));

    new CfnBudget(this, "Budget", {
      budget: {
        budgetName: `ncdb-${props.stage}-relay`,
        budgetType: "COST",
        timeUnit: "MONTHLY",
        budgetLimit: { amount: 20, unit: "USD" },
      },
      notificationsWithSubscribers: [50, 80, 100].map((threshold) => ({
        notification: {
          notificationType: "ACTUAL",
          comparisonOperator: "GREATER_THAN",
          threshold,
          thresholdType: "PERCENTAGE",
        },
        subscribers: [{ subscriptionType: "EMAIL", address: props.alarmEmail }],
      })),
    });

    new CfnOutput(this, "Url", {
      value: relay.url,
      description: "The relay's URL, which an app opens its database with",
    });
  }
}
