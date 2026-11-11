---
type: internal
tickets: [NOC-215]
components: [relay]
---
The relay core reads a space's control changes without loading its sealed frames, and reads the whole log in pages of at most 4 MiB of frames, so large changes do not make one request load gigabytes.
