---
type: internal
tickets: [NOC-194, NOC-185]
components: [core]
---
A test on both targets fails when the view's schema relies on a SQLite option one build lacks (a virtual table such as FTS or R-Tree, or a foreign key) or when what the view holds depends on the page size, as the owner decided on NOC-185 (A: the builds share the release, not the compile options).
