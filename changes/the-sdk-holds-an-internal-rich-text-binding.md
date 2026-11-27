---
type: internal
tickets: [NOC-461]
components: [sdk-ts]
---
The TypeScript SDK holds an internal rich-text binding: a Yjs document under a leased client id that writes at each pause, holds writes and pulls during an IME composition, reports its status and keeps one UndoManager; nothing of it is public yet.
