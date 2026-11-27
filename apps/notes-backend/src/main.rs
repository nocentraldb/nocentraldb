//! `notes-backend`: the example backend, run locally.
//!
//! ```text
//! notes-backend keygen
//! NOTES_RELAY_ID=<32 hex> NOTES_ADMISSION_KEY=<64 hex> [NOTES_LISTEN=127.0.0.1:8080] notes-backend
//! ```
//!
//! `keygen` draws an admission key and prints its secret, which the backend
//! keeps, and its public half, which the relay's operator puts in
//! `NCDB_ADMISSION_KEYS`. Without an argument it serves the HTTP API of
//! `http.rs` until it is stopped. The secret signs an admission for any
//! identity on that relay: an app keeps it where it keeps its other
//! secrets, never in its code or its logs.

use std::process::ExitCode;

use ed25519_dalek::SigningKey;
use notes_backend::{Backend, Config};

/// `N` bytes from their hex digits, or none.
fn hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != 2 * N || !text.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The variable `name`, as `N` bytes of hex; its value never appears in an
/// error.
fn var<const N: usize>(name: &str) -> Result<[u8; N], String> {
    let value = std::env::var(name).map_err(|_| format!("{name} is not set"))?;
    hex(&value).ok_or_else(|| format!("{name} is not {} hex digits", 2 * N))
}

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("keygen") {
        let mut secret = [0u8; 32];
        if getrandom::fill(&mut secret).is_err() {
            eprintln!("notes-backend: no secure random bytes");
            return ExitCode::FAILURE;
        }
        let key = SigningKey::from_bytes(&secret);
        println!("NOTES_ADMISSION_KEY={}", to_hex(&secret));
        println!(
            "NCDB_ADMISSION_KEYS={}",
            to_hex(key.verifying_key().as_bytes())
        );
        return ExitCode::SUCCESS;
    }
    let settings = var::<16>("NOTES_RELAY_ID")
        .and_then(|relay_id| Ok((relay_id, var::<32>("NOTES_ADMISSION_KEY")?)));
    let (relay_id, secret) = match settings {
        Ok(settings) => settings,
        Err(why) => {
            eprintln!("notes-backend: {why}");
            return ExitCode::from(2);
        }
    };
    let mut config = Config::new(relay_id, SigningKey::from_bytes(&secret));
    config.listen = std::env::var("NOTES_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    match Backend::start(config) {
        Ok(backend) => {
            println!("notes-backend listening on http://{}", backend.address());
            backend.wait()
        }
        Err(error) => {
            eprintln!("notes-backend: {error}");
            ExitCode::FAILURE
        }
    }
}
