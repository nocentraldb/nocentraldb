---
type: feature
tickets: [NOC-180, NOC-141]
components: [sdk-rust]
---
## Problem
An invitation had no text form, so an app could not carry a member's invitation from its device to an admin's: by a QR code, a link or a message.

## Use
`invitation.to_string()` writes `ncdb-invite-1:` and unpadded base64url of the byte layout RFC 0003 gives ("Invitation"); `text.parse::<Invitation>()` reads it back. A text that does not follow the layout is refused with `ErrorCode::InvalidValue`. Parsing checks only the layout; `members().add` checks the certificate.

```rust
let text = db.invitation().await.to_string();
// carry `text` to an admin's device, then:
let invitation: Invitation = text.parse()?;
let compared = invitation.fingerprint(); // a person compares it once
space
    .members()
    .add(&invitation, AddMember { role: Role::Writer, fingerprint: Some(compared) })
    .await?;
```
