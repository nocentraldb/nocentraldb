//! What the function reads from its environment, which the deployment
//! (`infra/aws`) sets:
//!
//! - `NCDB_RELAY_ID`: the relay's stable id, 32 hex digits (RFC 0007,
//!   "Opening a connection"), drawn once per deployment;
//! - `NCDB_ALLOWED_ACCOUNTS`: the account keys whose members may create
//!   spaces, 64 hex digits each, separated by commas (RFC 0007, "Who may
//!   create a space"); empty for none;
//! - `NCDB_TABLE_PREFIX`: the prefix of the backend's tables, as
//!   `ncdb_relay_aws::Names::with_prefix` names them, and of the
//!   `connections` table;
//! - `NCDB_BUCKET`: the bucket of frames above 8 KiB and of uploads;
//! - `NCDB_CONNECTIONS_URL`: the stage URL of the connection API,
//!   `https://<api id>.execute-api.<region>.amazonaws.com/<stage>`.
//!
//! A missing or malformed variable is refused by its name; its value never
//! appears in the error.

use std::collections::BTreeSet;
use std::fmt;

use ncdb_relay_aws::Names;

/// The function's settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The relay's stable id.
    pub relay_id: [u8; 16],
    /// The account keys whose members may create spaces.
    pub allowed_accounts: BTreeSet<[u8; 32]>,
    /// The backend's tables and bucket.
    pub names: Names,
    /// The `connections` table.
    pub connections: String,
    /// The stage URL of the connection API.
    pub connections_url: String,
}

/// A variable that is missing or malformed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused(pub &'static str);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the environment variable {} is missing or malformed",
            self.0
        )
    }
}

impl std::error::Error for Refused {}

fn hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != 2 * N || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0u8; N];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(bytes)
}

impl Settings {
    /// The settings `var` gives, such as `|name| std::env::var(name).ok()`.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Settings, Refused> {
        let need = |name: &'static str| {
            var(name)
                .filter(|value| !value.trim().is_empty())
                .ok_or(Refused(name))
        };
        let relay_id = hex(&need("NCDB_RELAY_ID")?).ok_or(Refused("NCDB_RELAY_ID"))?;
        let mut allowed_accounts = BTreeSet::new();
        for key in var("NCDB_ALLOWED_ACCOUNTS").unwrap_or_default().split(',') {
            if key.trim().is_empty() {
                continue;
            }
            allowed_accounts.insert(hex(key).ok_or(Refused("NCDB_ALLOWED_ACCOUNTS"))?);
        }
        let prefix = need("NCDB_TABLE_PREFIX")?;
        let mut names = Names::with_prefix(prefix.trim());
        names.bucket = need("NCDB_BUCKET")?.trim().to_owned();
        let connections = format!("{}connections", prefix.trim());
        let connections_url = need("NCDB_CONNECTIONS_URL")?.trim().to_owned();
        if !connections_url.starts_with("https://") {
            return Err(Refused("NCDB_CONNECTIONS_URL"));
        }
        Ok(Settings {
            relay_id,
            allowed_accounts,
            names,
            connections,
            connections_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| map.get(name).cloned()
    }

    const FULL: [(&str, &str); 5] = [
        ("NCDB_RELAY_ID", "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e"),
        (
            "NCDB_ALLOWED_ACCOUNTS",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        ),
        ("NCDB_TABLE_PREFIX", "ncdb-prod-"),
        ("NCDB_BUCKET", "ncdb-prod-frames-123456789012"),
        (
            "NCDB_CONNECTIONS_URL",
            "https://abc123.execute-api.eu-central-1.amazonaws.com/prod",
        ),
    ];

    #[test]
    fn the_settings_read_back_from_the_environment() {
        let settings = Settings::from_vars(vars(&FULL)).unwrap();
        assert_eq!(settings.relay_id, [0x5e; 16]);
        assert_eq!(
            settings.allowed_accounts,
            BTreeSet::from([[0xaa; 32], [0xbb; 32]])
        );
        assert_eq!(settings.names.log, "ncdb-prod-log");
        assert_eq!(settings.names.tokens, "ncdb-prod-tokens");
        assert_eq!(settings.names.bucket, "ncdb-prod-frames-123456789012");
        assert_eq!(settings.connections, "ncdb-prod-connections");
    }

    #[test]
    fn no_allowed_accounts_is_none() {
        let mut pairs = FULL.to_vec();
        pairs.retain(|(k, _)| *k != "NCDB_ALLOWED_ACCOUNTS");
        let settings = Settings::from_vars(vars(&pairs)).unwrap();
        assert!(settings.allowed_accounts.is_empty());
    }

    #[test]
    fn a_missing_or_malformed_variable_is_refused_by_name_only() {
        for (name, bad) in [
            ("NCDB_RELAY_ID", "5e5e"),
            ("NCDB_ALLOWED_ACCOUNTS", "not-a-key"),
            ("NCDB_CONNECTIONS_URL", "http://abc123.example/prod"),
            ("NCDB_BUCKET", " "),
        ] {
            let mut pairs = FULL.to_vec();
            pairs.retain(|(k, _)| *k != name);
            pairs.push((name, bad));
            let refused = Settings::from_vars(vars(&pairs)).unwrap_err();
            assert_eq!(refused, Refused(name));
            assert!(!refused.to_string().contains(bad.trim()) || bad.trim().is_empty());
        }
        let mut pairs = FULL.to_vec();
        pairs.retain(|(k, _)| *k != "NCDB_TABLE_PREFIX");
        assert_eq!(
            Settings::from_vars(vars(&pairs)).unwrap_err(),
            Refused("NCDB_TABLE_PREFIX")
        );
    }
}
