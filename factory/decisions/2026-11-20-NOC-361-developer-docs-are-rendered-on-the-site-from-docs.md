# 2026-11-20 · NOC-361 · Developer docs are rendered on the site from their files in docs/; contributor and operator docs stay in the repository

- Decided by: Carsten, 2026-11-19 on NOC-361 (relayed by pm): developer docs belong on the website, RFCs may stay GitHub links; which files are developer docs: the coding agent, by audience
- Why: one source, which the repository's checks keep compiling (the Rust guide's doc tests, the browser guide's type check), and no copy in `site/` to drift. `site/src/repo-docs.mjs` lists the rendered files: the two getting-started guides, browser durability, browser storage, the use cases, the FAQ and the glossary. A link from one of them to another becomes a site link, a link to any other file of the repository a GitHub link, and `site/scripts/docs-links.test.mjs` fails on any broken one. A new doc for app developers goes into docs/ and that list; release management, the relay's deploy, the repository, the AWS Emulator and the browser storage checklist are for contributors and operators and stay in the repository.
- Instead of: copies of the docs under `site/src/content/docs/`, or links from the site to the files on GitHub.
- Topics: site, docs
