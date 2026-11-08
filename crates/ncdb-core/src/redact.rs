//! `Debug` output that names sizes and kinds, never content (GENESIS.md:
//! "Never write content, keys or personal fields into logs, errors,
//! telemetry or crash reports").

use std::fmt;

/// Shows only a size: `<n unit>`.
pub(crate) struct Size(pub usize, pub &'static str);

impl fmt::Debug for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{} {}>", self.0, self.1)
    }
}

/// Shows that something is there, not what: `<redacted>`.
pub(crate) struct Hidden;

impl fmt::Debug for Hidden {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Bytes as lower-case hex, for ids, which are not content.
pub(crate) fn hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    bytes.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
}
