---
type: internal
tickets: [NOC-339]
components: [sdk-rust]
---
The Rust example app's README and the relay's deployment guide keep each device's options in a shell array and pass them with `"${LAPTOP[@]}"`, so their commands work in zsh, the default shell on macOS, as in bash. A string such as `LAPTOP="--data …"` stayed one word in zsh, and `tasks $LAPTOP create` failed.
