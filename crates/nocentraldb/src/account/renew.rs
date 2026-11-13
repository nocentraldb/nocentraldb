//! A new account key after a lost device (RFC 0010, "Losing a device",
//! step 3; NOC-268), the account space's side.

use ed25519_dalek::SigningKey;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::AccountKeyStatement;

use crate::db::Inner;
use crate::error::{Error, ErrorCode};

/// Draws a new account key with the number `number`.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn renew(
    _inner: &mut Inner,
    _recovery: &RecoveryKey,
    _number: u64,
) -> Result<(AccountKeyStatement, SigningKey), Error> {
    Err(Error::new(ErrorCode::InvalidValue, "not built yet"))
}

/// The newest account key the account space hands this device.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn new_key(_inner: &Inner) -> Result<Option<(AccountKeyStatement, SigningKey)>, Error> {
    Ok(None)
}

/// The member and number of each sealed account key the account space
/// holds.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn wrapped_to(_inner: &Inner) -> Result<Vec<([u8; 32], u64)>, Error> {
    Ok(Vec::new())
}
