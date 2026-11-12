//! What the member signs to authenticate a connection (RFC 0007, "Opening
//! a connection"): the SDK keeps its own copy of the digest, since the
//! relay's crate is native only and the browser signs too (NOC-175). The
//! two must be equal.

use crate::sync::wire::auth_digest;

#[test]
fn the_sdks_auth_digest_is_the_relays() {
    for (relay_id, nonce) in [
        (&[0x5e; 16][..], [7u8; 32]),
        (&[][..], [0; 32]),
        (&[1, 2, 3][..], [0xff; 32]),
    ] {
        assert_eq!(
            auth_digest(relay_id, &nonce),
            ncdb_relay::auth::digest(relay_id, &nonce)
        );
    }
}
