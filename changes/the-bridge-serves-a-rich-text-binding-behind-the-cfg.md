---
type: internal
tickets: [NOC-461]
components: [sdk-rust, sdk-ts]
---
The bridge answers a rich-text editor binding's lease, pull, write and close only with the cfg `ncdb_rich_text`, and the package's build refuses a core built with it.
