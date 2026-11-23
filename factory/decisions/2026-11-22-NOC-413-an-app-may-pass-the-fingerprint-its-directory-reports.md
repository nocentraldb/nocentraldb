# 2026-11-22 · NOC-413 · An app may pass the fingerprint its directory reports for a new account, and members.add then returns a notice that says so

- Decided by: Carsten, 2026-11-22, on RFC 0015's questions, through pm (NOC-429)
- Why: an app with open signup cannot make every pair of users compare 60 digits before working together. The app's backend then decides who a person is, and a backend that swaps an invitation adds a reader nobody chose, so the notice says the directory vouched for the account, and the app offers a screen to compare fingerprints later.
- Instead of: requiring a person to compare every new account's fingerprint.
- Topics: sdk-ts, sdk-rust, accounts, invitations, rfc-0015
