# tasks: a task list on two devices

A small command line app on NoCentralDB's Rust SDK. Two devices share a task list through a relay that cannot read it. You add a task on one and see it arrive on the other, live.

It shows, in about ten minutes:

- a schema written in TypeScript, and the `model.lock.json` that `ncdb model build` turns it into;
- a database on disk, a space, and tasks written and read through a declared index;
- a second device invited with a text invitation, after its fingerprint is confirmed;
- both devices syncing through a local relay, with live updates and the space's status.

It uses only the public Rust SDK (`nocentraldb`). Each run opens the device's database, does one thing and exits. When it wrote something, it first waits until the relay holds it. When the relay cannot be reached, the write stays on the device, the app says so, and a later `sync` brings it to the relay: the app works offline.

## What you need

- Rust, as `rust-toolchain.toml` pins it. `rustup` installs it on the first `cargo` call.
- Node.js 24 or later and pnpm, for the schema build.
- A clone of this repository. Run every command below from its root.

## 1. The schema and its lock

Install the JavaScript packages first, once: the schema build needs them.

```sh
pnpm install
```

The schema is `apps/tasks-rust/model.ts`: one collection, `tasks`, with a `title` and a `status` of `open` or `done`, and one index, `byStatus`, which a list reads. The app reads only the lock built from it, `apps/tasks-rust/model.lock.json`, which is checked in.

After you change the schema, build the lock again:

```sh
cd apps/tasks-rust && cargo run -q -p ncdb -- model build && cd ../..
```

`ncdb model check` tells you whether the lock is up to date. The build refuses a change that devices on the old lock could not keep, such as renaming a field.

## 2. Build the app and the relay

```sh
cargo build -p tasks-rust -p ncdb-relay-server
alias tasks=./target/debug/tasks
```

Each device below has its own database directory and key file. A key file keeps the device's keys in that file, for a machine without a keystore such as a container. Leave `--key-file` out to keep them in your platform's keystore.

```sh
LAPTOP="--data /tmp/laptop --key-file /tmp/laptop.keys --relay ws://127.0.0.1:7700"
PHONE="--data /tmp/phone --key-file /tmp/phone.keys --relay ws://127.0.0.1:7700"
```

## 3. Start a local relay

Start a relay on SQLite that lets the laptop's account create spaces. The relay names the account by the laptop's invitation, so no key is ever shown (`ncdb-relay --allow-invitation`, NOC-239):

```sh
./target/debug/ncdb-relay --database /tmp/relay.sqlite \
  --allow-invitation "$(tasks --data /tmp/laptop --key-file /tmp/laptop.keys invitation | head -1)"
```

It prints the address it listens on, `ws://127.0.0.1:7700`. Leave it running in its own terminal.

## 4. Create a space and add tasks

```sh
SPACE=$(tasks $LAPTOP create)
tasks $LAPTOP add $SPACE "Write the README"
tasks $LAPTOP list
```

`list` prints every task of every space on the device, open ones first:

```text
[ ] Write the README  (task 81624a8e…, space 16b70af4…)
```

## 5. Invite the second device

The second device prints its invitation, a line of text that holds its public keys and its account's certificate, and the fingerprint of its account:

```sh
tasks $PHONE invitation
```

Send the invitation line to the laptop in any way: it holds no secret. The laptop adds the phone to the space:

```sh
tasks $LAPTOP invite $SPACE "ncdb-invite-1:…"
```

The first time, the laptop refuses. The phone's account is new to it, and it prints the account's fingerprint. Compare that fingerprint with the one the phone printed, the way messaging apps compare safety numbers. Then run the command again with it:

```sh
tasks $LAPTOP invite $SPACE "ncdb-invite-1:…" --fingerprint "12345 67890 …"
```

The phone receives the space from the relay:

```sh
tasks $PHONE sync --space $SPACE
tasks $PHONE spaces
```

## 6. Live updates

In one terminal, let the phone follow the list. It prints the list again on every change:

```sh
tasks $PHONE watch
```

In another, add a task on the laptop:

```sh
tasks $LAPTOP add $SPACE "Buy milk"
```

The phone's terminal shows it within a moment. Complete it on the phone, using the task id that `list` shows, and the laptop sees it done:

```sh
tasks $PHONE done $SPACE <task id>
tasks $LAPTOP sync && tasks $LAPTOP list
```

Stop the relay and keep adding tasks on both devices. Each works on its own copy: a write succeeds and says where it is.

```text
saved on this device; the relay does not hold it yet, run `tasks sync` when it is back
```

When the relay is back, `sync` on each device brings both to the same list.

## 7. The space's status

```sh
tasks $PHONE status $SPACE
```

It prints the device's role in the space, whether its first sync is done, how far it is behind, and whether anything was parked, rejected or refused by the relay (RFC 0009, "Sync and status").

## How the code is laid out

- `src/main.rs` opens the database with `Db::open(OpenOptions::new(LOCK).path(..).key_file(..).relay(..))`.
- `src/commands.rs` holds one function per command: `db.spaces().create()`, `space.write(|tx| tx.create(..))`, `db.find(db.index("tasks", "byStatus")?)`, `db.subscribe(..)`, `space.members().add(..)` and `space.status()`.
- `src/wait.rs` waits until the relay holds this device's changes and its spaces are synced, following `db.sync().subscribe()` and `space.status().subscribe()`, and waits for a space another device shared with it through `db.spaces().subscribe()`.
- `tests/two_devices.rs` runs two instances of the app against a relay server, as the steps above do. It is part of `pnpm gates`.
