# notes-backend: admissions and a directory

The backend of a notes and projects app with open signup, as the guide [Cutting spaces, and invitations through your backend](../../docs/spaces-and-invitations.md) describes it. It does the two things a NoCentralDB app's backend does:

- **It admits its users.** It holds the app's admission key and signs an admission for the identity of each signed-in user's account, in RFC 0015's format ([RFC 0015, "Admission tokens"](../../docs/rfc/0015-relay-admission.md#admission-tokens)). The relay's operator puts the key's public half in `NCDB_ADMISSION_KEYS`.
- **It keeps a directory.** For each user it keeps the identity it admitted and the invitations of the user's devices, and it keeps the invites to a user who has no account yet, until the inviter's device carries them out.

It holds no key that reads content and cannot add anyone to a space: only an admin's device can. It uses no crate of NoCentralDB, so a backend in any language does the same.

| File | What it holds |
| --- | --- |
| `src/admission.rs` | The admission: Ed25519, SHA-256, base64url and a CBOR map of five keys. `tests/admission_vectors.rs` checks it against `docs/rfc/vectors/0015/`. |
| `src/directory.rs` | The directory and the pending invites. |
| `src/http.rs` | The HTTP API and its routes. |
| `src/main.rs` | The binary: `keygen`, and serving from the environment. |
| `model.ts`, `model.lock.json` | The app's model: `spaceInfo`, which says what each space is for, and `notes`. |
| `tests/journey.rs` | RFC 0015's user journey against the test relay, with each user's device on the Rust SDK. |

## Run its test

```sh
cargo test -p notes-backend
```

The journey test starts the Rust SDK's test relay with the backend's public admission key, so the relay takes no space from an account the backend did not admit. Ana signs up and is admitted, keeps a personal space and a project, and invites Ben before he has an account; once Ben signs up, Ana's device adds him, and Ben's device tells the project from his personal space by its `spaceInfo`. Last, Ana moves a note into a space she shares with Ben.

## Run it

```sh
cargo run -q -p notes-backend -- keygen
```

prints a new admission key: `NOTES_ADMISSION_KEY`, the secret the backend keeps, and `NCDB_ADMISSION_KEYS`, its public half for the relay. Then serve with the relay's id, `NCDB_RELAY_ID` of the relay you admit to:

```sh
NOTES_RELAY_ID=<32 hex digits> NOTES_ADMISSION_KEY=<64 hex digits> cargo run -q -p notes-backend
```

It listens on `127.0.0.1:8080`, or on `NOTES_LISTEN`.

**What it does not do.** It keeps the directory and the sessions in memory, so they are gone when it stops. Its signup takes a bare e-mail address and checks nothing: it stands in for your app's own signup and login, which decide who your users are. It serves plain HTTP on the loopback; your backend serves its routes behind its own login and TLS. The secret signs an admission for any identity on that relay: keep it where you keep your other secrets, never in your code or your logs. The local relay `ncdb-relay` takes no admission keys yet, so the backend's admissions work with the test relay and with the relay on AWS, not with `ncdb-relay`.
