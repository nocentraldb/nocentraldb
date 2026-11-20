# The landing page's TypeScript code

The TypeScript variant of each step of the landing page's code section
(`CodeSection.astro`), one section per step, named by the step's id; the
install step's line comes from `site/src/install.mjs`. The gates type-check
every block against the SDK's source (`scripts/landing-snippets.mjs`), so the
page cannot drift from `@nocentraldb/sdk`. Each block is a module of its own:
the model step's is `model.ts`, every other step's is `<step id>.ts` beside it.
Lines that start with `# ` are hidden, as rustdoc's are: they type-check, and
the page leaves them out. The Rust code is in `code-rust.md`.

## model

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  people: nc.collection({
    name: nc.string().personal({ person: "self" }),
  }),
  tasks: nc.collection(
    {
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      assignee: nc.link("people"),
    },
    {
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
      },
    },
  ),
});
```

## open

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "./model";
import lock from "./model.lock.json";

const db = await nc.open({ model, lock, relay: "wss://relay.example.com" });

const project = await db.spaces.create();
```

## share

```ts
# import { nc } from "@nocentraldb/sdk";
# import { model } from "./model";
# import lock from "./model.lock.json";
# declare function send(text: string): void;
# declare const text: string;
# const db = await nc.open({ model, lock });
# const project = await db.spaces.create();
// On the joining member:
const invitation = await db.invitation();
send(invitation.toString());

// On an admin of the space:
await project.members.add(nc.invitation(text), { role: "writer" });
```

## write

```ts
# import { nc } from "@nocentraldb/sdk";
# import { model } from "./model";
# import lock from "./model.lock.json";
# const db = await nc.open({ model, lock });
# const project = await db.spaces.create();
await project.write((tx) => {
  const task = tx.create(model.tasks, { title: "Write the README", status: "open", priority: 1 });
  tx.update(model.tasks, task, { status: "in_progress" });
});
```

## read

```ts
# import { nc } from "@nocentraldb/sdk";
# import { model } from "./model";
# import lock from "./model.lock.json";
# const db = await nc.open({ model, lock });
const query = model.tasks.indexes.byStatusPriority.equal("open").take(50);

const page = await db.find(query);
for (const task of page.items) console.log(task.title);
```

## subscribe

```ts
# import { nc } from "@nocentraldb/sdk";
# import { model } from "./model";
# import lock from "./model.lock.json";
# declare function render(page: unknown): void;
# const db = await nc.open({ model, lock });
const query = model.tasks.indexes.byStatusPriority.equal("open").take(50);

const stop = db.subscribe(query, (fresh) => render(fresh));
```
