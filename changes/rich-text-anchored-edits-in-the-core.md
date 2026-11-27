---
type: internal
tickets: [NOC-462]
components: [core]
---
The core makes an agent's anchored edits of a rich-text field, behind the compiler cfg `ncdb_rich_text`: the read version and its cache, the lookup of a quote in the version read, and the smallest change written on the Yrs document under the agent's lease (RFC 0013, step 5).
