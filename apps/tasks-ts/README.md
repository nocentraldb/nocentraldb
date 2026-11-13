# tasks-ts: a task list in the browser

A small web page on NoCentralDB's TypeScript SDK (`@nocentraldb/sdk`). The page keeps its tasks in a database in the browser, on the origin private file system (OPFS). Each list on the page is a live query: you add a task or tick one done, and the lists show the change without the page asking again.

It shows, in about five minutes:

- a schema written in TypeScript, and the `model.lock.json` that `ncdb model build` turns it into;
- a database opened in the browser with `nc.open`, which runs the core in a worker the SDK starts itself;
- tasks created, updated and deleted in a space;
- two lists, each a subscription to a query of a declared index, a page at a time.

It runs on one device. The TypeScript SDK cannot sync yet; the Rust example, `apps/tasks-rust`, shows two devices syncing through a relay.

## What you need

- Rust, as `rust-toolchain.toml` pins it, to compile the core to WebAssembly. `rustup` installs it on the first `cargo` call.
- Node.js 24 or later and pnpm.
- A browser with OPFS. The app is tested in Chromium.
- A clone of this repository. Run every command below from its root.

## 1. The schema and its lock

The schema is `apps/tasks-ts/model.ts`: one collection, `tasks`, with a `title` and a `status` of `open` or `done`, and one index, `byStatus`. The index orders tasks by status, then title. Its option `equal: 1` lets a query fix the status, so each list reads one status.

The app opens its database with the lock built from the schema, `apps/tasks-ts/model.lock.json`, which is checked in. After you change the schema, build the lock again. The build imports the SDK from its build output, so build the SDK's TypeScript first:

```sh
pnpm install
node packages/sdk/build.mjs --ts-only
cd apps/tasks-ts && cargo run -q -p ncdb -- model build && cd ../..
```

## 2. Build the app

```sh
node apps/tasks-ts/build.mjs
```

This builds the SDK first, with its core compiled to WebAssembly, into `packages/sdk/dist/`. Then it compiles the app's TypeScript with `tsc` into `apps/tasks-ts/dist/`. To build only the app again after a change to it, add `--app-only`.

The app needs no bundler. The page maps the package name `@nocentraldb/sdk` to the SDK's build output with an import map, and loads the compiled app as an ES module.

## 3. Open it

```sh
node apps/tasks-ts/server.mjs
```

Open `http://localhost:8336/`. A browser grants OPFS only to a secure context, and `localhost` is one.

Add a few tasks. Each one shows under **Open**, in title order. Tick one and it moves to **Done**. Untick it and it moves back. **Delete** removes it. Reload the page: the tasks are still there, read from OPFS.

Open `http://localhost:8336/?page=2` to see a list a page at a time. Each list shows two tasks, and **Show more** shows two more.

## How the code is laid out

- `model.ts` is the schema, and `model.lock.json` is its lock.
- `src/app.ts` opens the database with `nc.open({ model, lock, name: "tasks" })`. It uses the device's first space, or `db.spaces.create()` when there is none. It writes with `space.create`, `space.update` and `space.delete`. It follows each list with `db.subscribe(model.tasks.indexes.byStatus.equal("open").take(n), listener)`, and redraws the list from each page the listener receives.
- `src/tasks.ts` holds the logic without the DOM: the lines of a list, the status a tick writes, the title an input adds, and the page size. `test/tasks.test.ts` tests it with vitest, in `pnpm gates`.
- `index.html` is the page, and `server.mjs` serves it on localhost.
- `e2e/tasks.e2e.mjs` drives the page in Chromium through Playwright, as the steps above do. The task `tasks-ts:browser` runs it locally, and the workflow `browser-view` runs it in CI. It is not part of `pnpm gates`, which runs without browsers.
