//! `ncdb-relay`: the relay server on the SQLite backend, run locally.
//!
//! ```text
//! ncdb-relay --database <path> [--listen <address>] [--public-url <url>] [--allow-account <hex>]...
//! ```
//!
//! It serves plain WebSocket (`ws://`) on `--listen`, by default
//! `127.0.0.1:7700`, which must be a loopback address; TLS, which RFC 0007
//! requires on the network, is left to a proxy on the same host. `--allow-account` names, as 64 hex digits,
//! an account key whose members may create spaces (RFC 0007, "What the
//! relay checks"). `--public-url` is where members reach the server's
//! upload and download URLs for large changes, such as
//! `https://relay.example.com` behind a TLS proxy; by default `http://` and
//! the address it listens on.
//! The relay id lives next to the database, in `<path>.relay-id`, and is
//! drawn on the first start. It prints one line, the address it listens on.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ncdb_relay::Relay;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay_server::{Server, config};

const USAGE: &str = "usage: ncdb-relay --database <path> [--listen <address>] [--public-url <url>] \
     [--allow-account <hex>]...";

/// What the command line asks for.
struct Args {
    database: PathBuf,
    listen: String,
    public_url: Option<String>,
    allowed: BTreeSet<[u8; 32]>,
}

fn parse(mut args: impl Iterator<Item = String>) -> Option<Args> {
    let (mut database, mut listen, mut allowed) = (None, None, BTreeSet::new());
    let mut public_url = None;
    while let Some(arg) = args.next() {
        let value = args.next()?;
        match arg.as_str() {
            "--database" => database = Some(PathBuf::from(value)),
            "--listen" => listen = Some(value),
            "--public-url" => public_url = Some(value),
            "--allow-account" => {
                allowed.insert(account(&value)?);
            }
            _ => return None,
        }
    }
    Some(Args {
        database: database?,
        listen: listen.unwrap_or_else(|| "127.0.0.1:7700".to_owned()),
        public_url,
        allowed,
    })
}

/// An account key written as 64 hex digits.
fn account(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.is_ascii() {
        return None;
    }
    let mut key = [0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(key)
}

/// The relay id stored next to the database, drawn on the first start.
fn relay_id(database: &Path) -> Result<[u8; 16], String> {
    let mut path = database.as_os_str().to_owned();
    path.push(".relay-id");
    let path = PathBuf::from(path);
    match std::fs::read(&path) {
        Ok(bytes) => bytes
            .try_into()
            .map_err(|_| format!("{} does not hold 16 bytes", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut id = [0u8; 16];
            getrandom::fill(&mut id).map_err(|e| format!("no secure random bytes: {e}"))?;
            std::fs::write(&path, id).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(id)
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let Some(args) = parse(std::env::args().skip(1)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ncdb-relay: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<(), String> {
    let relay_id = relay_id(&args.database)?;
    let store = SqliteStore::open(&args.database).map_err(|e| e.to_string())?;
    let relay = Relay::new(store, config(args.allowed));
    let mut server = Server::bind(&args.listen, relay, relay_id)
        .await
        .map_err(|e| format!("{}: {e}", args.listen))?;
    if let Some(url) = &args.public_url {
        server = server.with_base_url(url);
    }
    let addr = server.local_addr().map_err(|e| e.to_string())?;
    println!("ncdb-relay listening on ws://{addr}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    server.run().await.map_err(|e| e.to_string())
}
