//! The account key of a member's invitation, for `--allow-invitation`
//! (NOC-257; the owner's NOC-239 A, Carsten, 2026-11-12). An invitation's
//! text is `ncdb-invite-1:` and unpadded base64url of RFC 0003's layout
//! ("Invitation"); the account key is key 1 of the member's certificate, at
//! offset 65. A developer who uses only the public SDK gets that text from
//! their app's invitation, which never shows a key.
//!
//! The text is checked as RFC 0003 has every reader check it, and refused
//! when it is not an invitation. The relay never verifies the certificate:
//! it admits an account, which the member's own connection proves later.
//! A refusal says why, and never repeats the text or any key from it. The
//! SDK's reader is crate-internal and the core has none, so this one keeps
//! to the vectors of `docs/rfc/vectors/0003/`, as the SDK's does.

/// What every invitation's text starts with.
const PREFIX: &str = "ncdb-invite-1:";

/// The layout's version byte.
const VERSION: u8 = 1;

/// The length of the layout without an account key statement, and with one.
const PLAIN: usize = 163;
const WITH_STATEMENT: usize = 267;

/// The account key the invitation `text` names, or why `text` is not an
/// invitation.
pub(crate) fn account_key(text: &str) -> Result<[u8; 32], &'static str> {
    let body = text
        .strip_prefix(PREFIX)
        .ok_or("it does not start with ncdb-invite-1:")?;
    let bytes = base64url(body).ok_or("it is not unpadded base64url")?;
    match bytes.first() {
        Some(&VERSION) => {}
        Some(_) => return Err("its version is not 1"),
        None => return Err("it is empty"),
    }
    if bytes.len() < PLAIN {
        return Err("it is shorter than an invitation");
    }
    if !matches!(bytes[97], 1 | 2) {
        return Err("its kind is neither 1 (device) nor 2 (agent)");
    }
    match (bytes[162], bytes.len()) {
        (0, PLAIN) => {}
        (1, WITH_STATEMENT) => {
            if bytes[195..203] == [0; 8] {
                return Err("its account key number is 0");
            }
        }
        (0 | 1, _) => return Err("its length is not the one its statement byte gives"),
        _ => return Err("its statement byte is neither 0 nor 1"),
    }
    Ok(bytes[65..97].try_into().expect("32 bytes"))
}

/// The bytes of unpadded base64url (RFC 4648, section 5), strictly: no
/// padding, no other character, and unused trailing bits zero, so each
/// sequence of bytes has one text.
fn base64url(text: &str) -> Option<Vec<u8>> {
    if text.len() % 4 == 1 {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() * 3 / 4);
    let (mut held, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        };
        held = held << 6 | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((held >> bits) as u8);
            held &= (1 << bits) - 1;
        }
    }
    (held == 0).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::{account_key, base64url};

    const VECTOR: &str = include_str!("../../../docs/rfc/vectors/0003/invitation.txt");
    const VECTOR_STATEMENT: &str =
        include_str!("../../../docs/rfc/vectors/0003/invitation-statement.txt");
    const FIELDS: &str = include_str!("../../../docs/rfc/vectors/0003/fields.txt");

    /// The vectors' account key, from `fields.txt`.
    fn vector_account() -> [u8; 32] {
        let hex = FIELDS
            .lines()
            .find_map(|line| line.strip_prefix("account key: "))
            .expect("fields.txt names the account key");
        let bytes: Vec<u8> = (0..64)
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        bytes.try_into().unwrap()
    }

    #[test]
    fn the_vectors_name_their_account_key() {
        assert_eq!(account_key(VECTOR.trim()), Ok(vector_account()));
        assert_eq!(account_key(VECTOR_STATEMENT.trim()), Ok(vector_account()));
    }

    #[test]
    fn base64url_reads_rfc_4648s_examples_without_padding() {
        // RFC 4648, section 10, in the URL-safe alphabet, unpadded.
        for (text, bytes) in [
            ("", &b""[..]),
            ("Zg", b"f"),
            ("Zm8", b"fo"),
            ("Zm9v", b"foo"),
            ("Zm9vYg", b"foob"),
            ("Zm9vYmE", b"fooba"),
            ("Zm9vYmFy", b"foobar"),
            ("-_8", &[0xfb, 0xff]),
        ] {
            assert_eq!(base64url(text).as_deref(), Some(bytes), "{text}");
        }
    }

    #[test]
    fn base64url_refuses_padding_other_characters_and_unused_bits() {
        for text in ["Zg==", "Zm8=", "Z", "Zm9v+", "Zm9v/", "Zm 9v", "Zh", "Zm9"] {
            assert_eq!(base64url(text), None, "{text}");
        }
    }
}
