# 2026-11-23 · NOC-417 · The `Relay` construct adds its alarms only on `addAlarms`, and holds no budget

- Decided by: coding agent (NOC-417)
- Why: where alarms go and what a relay may cost are the app's choices. `addAlarms()` adds the four alarms (errors, throttles, an unfinished sweep, an overdue sweep pass) and returns the SNS topic they notify; the 20 USD budget, the alarm mail and the URL output stay in `RelayStack`. The construct's public properties are `api`, `stage`, `connectRoute`, `handler`, `logGroup` and `url`.
- Instead of: alarms and a budget in every construct, which would mail and bill by the project's numbers; an alarm-action prop, which would move the topic out of the construct and reorder the project's template.
- Topics: relay, aws, cdk
