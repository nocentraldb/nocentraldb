# Cutting spaces, and invitations through your backend

A space is what your app shares: every member of a space reads everything in it, and nobody else can. So how you cut your data into spaces decides who can read what. This guide shows how to cut them, how a device tells its kinds of spaces apart, and how an app with open signup admits its users and carries invitations through its own backend. It follows one user of a notes and projects app, from signup to working with others, as [RFC 0015, "A user's journey"](rfc/0015-relay-admission.md#a-users-journey) does.

The example backend in [apps/notes-backend/](../apps/notes-backend/) does what the backend does here: it signs admissions and keeps a directory. Its test runs the whole journey against a relay, with each user's device on the Rust SDK ([Run the example](#run-the-example)). The snippets use the TypeScript SDK; the Rust SDK has the same calls in snake case.

## Cut by who reads

**A space holds the objects that the same members read.** Cut by who reads, not by object type. Adding a member to a space shares everything in that space: every object in it and its whole history, from the space's first change on. `members.add` says so in its notice, `memberAdded`: "The new member reads the whole history of this space." There is no right finer than a space, so an object that some members of a space must not read belongs in another space.

Removing a member does not take back what it read: it keeps its copy of everything it received before the removal, and the notice `memberRemoved` says so.

An app of notes and projects uses these kinds of spaces:

| Space | Members | Holds | Created |
| --- | --- | --- | --- |
| personal space | The user's own devices | The user's private notes, settings and drafts | By the app, once per user |
| project space | The project's members, each with a role | The project's name, its tasks and its notes | By the app, once per project |
| shared-note space | The people a note is shared with | One note, or a few notes shared with the same people | By the app, when a note is shared on its own, or when the note is created if the app expects that |
| account space | The user's devices, each an admin | What the SDK needs to keep the account's devices together; no data of your app | By the SDK |

An agent that hands a task to a sub-agent creates an ad-hoc space as well, with only what the task needs ([RFC 0009, "Sub-agents and ad-hoc spaces"](rfc/0009-sdk-api.md#sub-agents-and-ad-hoc-spaces)).

- **Not too coarse.** One space for all of an app's data gives every member every object, and one space per team makes every member of the team a reader of every project.
- **Not too fine.** Every space costs work when membership changes: adding a person writes an entry and a key envelope in each space, and removing one rotates the key of each space, on the admin's device. A space per task in a project of 300 tasks makes adding a member 300 adds. Split a space when its readers differ, not when its objects do.
- **Across spaces, use references.** A link points only into its own space. A reference (`nc.ref`) points into another space, and reads as `notAccessible` for a member who is not in the target's space ([RFC 0004, "References"](rfc/0004-merge-rules-basic-types.md#references)). A project lists its shared notes by reference.

## Tell your spaces apart

A space has no name and no kind in the SDK. Its first change holds no name, because a name is content the relay must not read ([RFC 0009, "Spaces"](rfc/0009-sdk-api.md#spaces)). Your app says what a space is with an object of its own model, written into the space when the app creates it. In this app it is a `spaceInfo` object with a `kind` and a `name`:

```ts model.ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  spaceInfo: nc.collection(
    {
      kind: nc.oneOf(["personal", "project", "note"]),
      name: nc.string(),
    },
    { indexes: { byKind: nc.index(["kind", "name"], { equal: 1 }) } },
  ),
  notes: nc.collection(
    { title: nc.string() },
    { indexes: { byTitle: nc.index(["title"]) } },
  ),
});
```

Create a space and its `spaceInfo` together:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "notes" });

const project = await db.spaces.create();
await project.create(model.spaceInfo, { kind: "project", name: "Launch" });
await project.create(model.notes, { title: "Plan" });
```

A device finds its spaces of each kind through the index, and each object it reads names its space in `$space`. `db.spaces.list()` gives every app space the device holds, with its role; `db.spaces.subscribe` follows that list, for example when another device adds this one to a project:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "notes" });

const projects = await db.find(
  model.spaceInfo.indexes.byKind.equal("project").take(50),
);
for (const info of projects.items) {
  console.log(`project ${info.name ?? ""} in space ${info.$space}`);
}

const kinds = ["personal", "project", "note"] as const;
const stop = db.spaces.subscribe(async (spaces) => {
  const known = new Set<string>();
  for (const kind of kinds) {
    const page = await db.find(
      model.spaceInfo.indexes.byKind.equal(kind).take(500),
    );
    for (const info of page.items) known.add(info.$space);
  }
  for (const space of spaces) {
    if (space.model === "ours" && !known.has(space.id)) {
      console.log(`space ${space.id} is still arriving`);
    }
  }
});
console.log((await db.spaces.list()).length, typeof stop);
```

- **The account space is the SDK's.** It holds the account's devices and agents, its account key statements and which spaces each device belongs to, in a model the SDK ships ([RFC 0010, "The account space"](rfc/0010-accounts.md#the-account-space)). `db.spaces.list()` does not list it, your app cannot write into it, and your queries never return its objects. Your personal space is a space of your own, not the account space.
- **Create the personal space once, on the device that creates the account.** A device your app opens with `pairing: "request"` joins the account instead: pairing adds it to every space where a device of the account is an admin, the personal space included, so it waits for that space to arrive rather than creating another ([RFC 0010, "Adding a device or an agent to an account"](rfc/0010-accounts.md#adding-a-device-or-an-agent-to-an-account)).
- **A space can arrive before its `spaceInfo`.** A device lists a space once it holds the space's first change and its key; the objects follow as they sync. Show such a space as arriving, not as one without a kind.
- **`spaceInfo` is an object like any other.** Every writer of the space can change it, and a member of the space reads it. Whoever may not rename a project is a reader of its space.
- **A space of another model** shows in the list with `model: "another"`; its objects are not read, and `db.spaces.get` on it fails with `spaceOfAnotherModel`.

## Admit your users

A relay lets only the accounts it admits create spaces. An app with open signup cannot list its users when the relay is deployed, so its backend admits them: it signs an **admission** for an account's identity with the app's **admission key**, an Ed25519 key that only the backend holds ([RFC 0015, "Admission tokens"](rfc/0015-relay-admission.md#admission-tokens)). The relay's operator configures the public half of the key once, in `NCDB_ADMISSION_KEYS` ([the relay's deploy guide](relay-deploy.md)).

Pass the SDK a function that fetches an admission from your backend. The SDK calls it when the relay refuses a new space with `not_allowed`, presents the answer, and pushes the space again, once:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({
  model,
  lock,
  name: "notes",
  relay: "wss://relay.example.com",
  admission: (identity) =>
    fetch("/api/admission", { method: "POST", body: identity }).then((r) =>
      r.text(),
    ),
});

// Tell the directory this device's invitation, so that others can add it.
const invitation = await db.invitation();
await fetch("/api/me/invitations", {
  method: "PUT",
  body: invitation.toString(),
});
console.log(db.account.identity);
```

The function gets the account's identity, `db.account.identity`: its recovery key in hexadecimal, public, the same on every device of the account and after a recovery. Your backend checks that the caller is signed in as a user of the app, records the identity for that user, and answers the admission's text, `ncdb-admit-1:…`. Your code handles a string from your own backend, never a key.

An admission names the relay's id, the identity, the time it lapses and how many spaces the identity may create. The example signs it in [apps/notes-backend/src/admission.rs](../apps/notes-backend/src/admission.rs): Ed25519, SHA-256, base64url and a CBOR map of five keys, which a backend in any language builds the same way, and checks against the vectors in [docs/rfc/vectors/0015/](rfc/vectors/0015/).

- **Count every space.** The number of spaces an admission grants covers the account space, the personal space and every project and shared note the user will create. The relay cannot tell them apart.
- **Choose when it lapses.** When an admission has lapsed, the SDK fetches a new one through the same function the next time the relay refuses a new space. Your backend withdraws a user by no longer signing for them. The user's spaces keep working: an admission decides only who creates spaces.
- **A new admission keeps the count.** A later admission for the same identity replaces the earlier one, and the spaces the identity created still count against it.

What admission does not do: it does not decide who your users are, which your signup does, with whatever it uses against fake accounts. It limits how many spaces an identity creates, not how much it writes into them. A leaked admission key admits any identity on that relay until the operator removes the key. A member can present an older admission that has not lapsed yet; a short lapse time bounds that.

## Keep a directory

The backend is where an account's identity meets your app's user, so it keeps a **directory**: for each user, such as an e-mail address, the identity it admitted and the invitations of the user's devices. An invitation is a member's public identity: its member id, public keys and certificate ([RFC 0009, "Invitations and members"](rfc/0009-sdk-api.md#invitations-and-members)). Nothing in the directory is secret, and the relay sees identities and invitations anyway. Each device sends its invitation after it opens, as the snippet above does, and a device the user pairs later sends its own.

An invitation flows from the joining member to an admin, never the other way: an admin's device adds a member only with that member's invitation, and only a device that holds the space key can write the add. The backend holds no space key, so it can never add anyone. It carries invitations from one user's devices to another user's.

## Invite someone to a project

Ana invites Ben by his e-mail address. Her device asks the backend for Ben's invitations, one per device, and adds them all:

```ts
import { nc } from "@nocentraldb/sdk";
import { model } from "../model.ts";

const lock: unknown = await (await fetch("/model.lock.json")).json();
const db = await nc.open({ model, lock, name: "notes" });
const project = await db.spaces.get(new URLSearchParams(location.search).get("space") ?? "");

const { invitations } = (await (
  await fetch("/api/users/ben@example.com/invitations")
).json()) as { invitations: string[] };
const devices = invitations.map((text) => nc.invitation(text));
try {
  await project.members.add(devices, { role: "writer" });
} catch (error) {
  if (!(error instanceof nc.Error) || error.code !== "unknownAccount") {
    throw error;
  }
  // Ben's account is new to this device: compare its fingerprint, or pass
  // the one the directory vouches for, as your app decides.
  const added = await project.members.add(devices, {
    role: "writer",
    fingerprint: error.fingerprint ?? "",
  });
  console.log(added.notice.text);
}
```

**The first add of an account needs its fingerprint.** Ben's account is new to Ana's device, so `members.add` fails with `unknownAccount`, which carries the account's fingerprint, until the app passes it. A person can compare it on both screens, as messaging apps compare safety numbers. An app with a directory may instead pass the fingerprint of the invitation its directory gave, as its own choice. Then your backend decides who Ben is: a backend that swapped Ben's invitation for one of its own would add a reader nobody chose. Give your users a screen where they compare fingerprints later. The SDK does not yet return a notice that says an add relied on the directory. After the first add, the device knows Ben's account, and a later add of any of his devices needs no fingerprint.

Ben's device learns of the project at once if it is connected, and otherwise at its next connection: `db.spaces.subscribe` calls its listener with the new list. Your backend can also send Ben an e-mail or a push notification; it learns no content.

Ben's agents are not in the list: an agent gets only the spaces its task needs, and Ben adds it himself.

## Invite someone who has no account yet

When Ben has no account, nobody can add him: a member is a key pair on a device, and Ben has no device with your app yet. The invitation is a promise your app keeps, not a change in any space:

1. Ana invites ben@example.com. The backend stores a pending invite (the project's space and the role) and sends Ben a link to the app.
2. Ben signs up and opens the app. His device creates his account, and its first space brings his identity to the backend; his device sends its invitation.
3. The backend tells Ana's devices that the invite can be carried out, by a push notification or when they next ask.
4. A device of Ana's, or of any admin of the project, fetches Ben's invitations, adds him, and tells the backend the invite is done.

Between steps 2 and 4, Ben has signed up but sees nothing of the project, until a device of an admin of the project runs. That gap is the price of an invitation that flows from the joining member: no server may write an add, because it would hold the space key. An app may close it with an admin agent of the inviter's account that runs in the cloud, as a visible member of the project, and carries out pending invites while Ana's devices are off.

The pending invite tells your backend who was invited to which project. The relay never learns that; your backend knows it already as your app's own data.

## Share a single note

A note lives in exactly one space, and membership is per space, so Ben cannot be given one note of a project, or of Ana's personal space, alone. Share it through a space of its own: create a shared-note space, add Ben, and move the note into it. A move keeps the note's id; the members of the old space keep its history up to the move, and Ben reads it from the move on ([RFC 0016, "The SDK call and its notice"](rfc/0016-moving-an-object.md#the-sdk-call-and-its-notice)). The Rust SDK has the move; the TypeScript SDK's `space.move` comes later:

```rust no_run
use nocentraldb::{AddMember, Db, Error, Invitation, ObjectId, Role, Space, Value};

/// Shares the note `note` of `personal` with Ben's devices, `ben`.
async fn share(db: &Db, personal: &Space, note: ObjectId, ben: &[Invitation]) -> Result<(), Error> {
    let shared = db.spaces().create().await?;
    shared
        .create("spaceInfo", [("kind", Value::from("note")), ("name", "For Ben".into())])
        .await?;
    for invitation in ben {
        // Ben's account is known to this device since the project.
        let options = AddMember { role: Role::Writer, fingerprint: None };
        shared.members().add(invitation, options).await?;
    }
    if let Some(object) = personal.get("notes", note).await? {
        let moved = personal.move_objects(&[object], &shared).await?;
        println!("{}", moved.notice.text);
    }
    Ok(())
}
```

An app that expects notes to be shared on their own can create each such note in its own space from the start, and keep a reference to it in the project.

## Run the example

[apps/notes-backend/](../apps/notes-backend/) is a backend of this app in a few hundred lines of Rust, with no crate of NoCentralDB: an HTTP API that signs admissions ([src/admission.rs](../apps/notes-backend/src/admission.rs)) and a directory with pending invites ([src/directory.rs](../apps/notes-backend/src/directory.rs)). Its test, [tests/journey.rs](../apps/notes-backend/tests/journey.rs), runs this guide's journey: Ana is admitted through the backend, keeps a personal space and a project, invites Ben before he has an account, adds him once he signs up, and moves a note into a space she shares with him; Ben's device tells the project from his personal space by its `spaceInfo` and never lists the account space. The relay in the test is the Rust SDK's test relay, which takes only the admissions the backend's key signed:

```sh
cargo test -p notes-backend
```

The example keeps its directory in memory and stands in for your signup with a bare e-mail address; your backend keeps the directory in its own database, behind its own login and TLS. The local relay `ncdb-relay` takes no admission keys yet, so it admits accounts only by `--allow-invitation`; the test relay and the relay on AWS take admissions.
