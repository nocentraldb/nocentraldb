# Security policy

## Reporting a vulnerability

Report a vulnerability through GitHub private vulnerability reporting: on the repository, open **Security**, then **Report a vulnerability** ([direct link](https://github.com/nocentraldb/nocentraldb/security/advisories/new)).

Never report a vulnerability in a public issue, discussion or pull request. A public report puts every user at risk before a fix exists. If you are unsure whether something is a vulnerability, report it privately anyway.

Please include what is affected, how to reproduce it, and what an attacker gains. The maintainers read the reports. You get an answer on the report itself, and we agree with you on a disclosure date once a fix is ready.

## Supported versions

Before 0.1.0 only the default branch `main` and the latest release are supported. Fixes are not backported.

## What is not a vulnerability

These reports are closed without an advisory:

- **Behaviour of a local replica under the user's own control.** A process running as the user can read what the user can read; NoCentralDB is not a security boundary on the user's own device (RFC 0001, What it is not). Reading or changing a local replica with the user's own keys is expected.
- **The relay's metadata.** The relay cannot read content, but it sees which key is in which space, and when and how much is written (RFC 0001, The design in one page). That is documented behaviour, not a vulnerability. A way for the relay to learn more than that is a vulnerability.
- **Test and debug code.** Code under tests, fixtures, examples and debug-only builds that does not ship in a release.
- **A dependency CVE on its own.** A report that only cites a CVE in a dependency, without showing that NoCentralDB reaches the vulnerable code in a way an attacker can use.
- **Findings from a scanner without a demonstrated impact**, such as missing headers on a page or a theoretical weakness with no path to exploit it.

See [RFC 0001](../docs/rfc/0001-nocentraldb.md#the-design-in-one-page) for the design these statements rest on.
