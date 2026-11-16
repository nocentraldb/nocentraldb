---
type: fix
tickets: [NOC-340]
introduced: cycle
components: [sdk-rust]
---
## Symptom

The Rust example app `tasks` panicked with "failed printing to stdout: Broken pipe" and a backtrace when the program it wrote to stopped reading, as in the README's `tasks … invitation | head -1`.

## When

Whenever a reader closed `tasks`'s output before `tasks` was done writing, such as `head`. The output the reader took was correct.

## Cause

Rust's `println!` panics when stdout is a closed pipe.

## Fix

`tasks` writes its output through one helper, which ends the app quietly with status 0 when nothing reads its output any more, as command-line tools do.
