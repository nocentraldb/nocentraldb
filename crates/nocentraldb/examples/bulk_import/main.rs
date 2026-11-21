//! `bulk_import`: measures a burst of writes into one space, such as an
//! import (NOC-414). An example of the SDK, not part of it; it uses only
//! the SDK's public API.
//!
//! ```text
//! cargo build --release --locked -p ncdb-relay-server --bin ncdb-relay
//! cargo run --release --locked -p nocentraldb --example bulk_import -- \
//!   [--operations <n>] [--batch <creates per change>] [--island] \
//!   [--relay-bin <path>] [--dir <path>]
//! ```
//!
//! It starts the local relay (`ncdb-relay`, by default the one in the same
//! target directory) and writes `n` synthetic records into a new space of a
//! first device, `batch` creates to one change. By default, the first
//! device writes online, waits until the relay holds the burst, and then
//! pairs a second device of the same account, which catches up as a new
//! device. With `--island`, the second device is paired first and goes
//! offline; the first device writes the burst offline, reconnects and
//! pushes it; then the second device comes back and pulls it. It prints
//! what each step took and what the relay received.
//! Everything lives in a temporary directory, removed at the end, or in
//! `--dir`, a new directory that stays for a look at the databases.
//! `docs/bulk-import.md` explains the numbers.

mod catch_up;
mod device;
mod import;
mod memory;
mod relay;
mod second;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use second::Second;

const USAGE: &str = "usage: bulk_import [--operations <n>] [--batch <creates per change>] \
     [--island] [--relay-bin <path>] [--dir <path>]";

/// What the command line asks for.
struct Args {
    operations: u64,
    batch: u64,
    island: bool,
    relay_bin: Option<PathBuf>,
    dir: Option<PathBuf>,
}

fn parse(mut args: impl Iterator<Item = String>) -> Option<Args> {
    let mut parsed = Args {
        operations: 81_000,
        batch: 500,
        island: false,
        relay_bin: None,
        dir: None,
    };
    while let Some(arg) = args.next() {
        if arg == "--island" {
            parsed.island = true;
            continue;
        }
        let value = args.next()?;
        match arg.as_str() {
            "--operations" => parsed.operations = value.parse().ok().filter(|&n| n > 0)?,
            "--batch" => parsed.batch = value.parse().ok().filter(|&n| n > 0)?,
            "--relay-bin" => parsed.relay_bin = Some(PathBuf::from(value)),
            "--dir" => parsed.dir = Some(PathBuf::from(value)),
            _ => return None,
        }
    }
    Some(parsed)
}

/// A directory of its own for one run, removed when dropped unless it
/// was asked to stay.
struct TempDir(PathBuf, bool);

impl TempDir {
    /// The new directory `kept`, which stays.
    fn kept(kept: PathBuf) -> Result<TempDir, String> {
        std::fs::create_dir(&kept).map_err(|e| format!("{}: {e}", kept.display()))?;
        Ok(TempDir(kept, true))
    }

    fn new() -> Result<TempDir, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("bulk-import-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(TempDir(dir, false))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.1 {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

/// A database's size: its file, and its write-ahead log.
fn database((file, wal): (u64, u64)) -> String {
    format!("{} (+ {} write-ahead log)", megabytes(file), megabytes(wal))
}

fn seconds(millis: u128) -> String {
    format!("{:.1} s", millis as f64 / 1000.0)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    let result = if args.peek().map(String::as_str) == Some("catch-up") {
        args.next();
        match (args.next(), args.next(), args.next()) {
            (Some(dir), Some(relay), Some(mode)) => {
                catch_up::run(&PathBuf::from(dir), &relay, &mode).await
            }
            _ => Err(USAGE.to_owned()),
        }
    } else {
        match parse(args) {
            Some(args) => measure(args).await,
            None => Err(USAGE.to_owned()),
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bulk_import: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn measure(args: Args) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // target/<profile>/examples/bulk_import → target/<profile>/ncdb-relay
    let relay_bin = match args.relay_bin {
        Some(path) => path,
        None => exe
            .parent()
            .and_then(std::path::Path::parent)
            .ok_or("no target directory")?
            .join(format!("ncdb-relay{}", std::env::consts::EXE_SUFFIX)),
    };
    let dir = match args.dir {
        Some(kept) => TempDir::kept(kept)?,
        None => TempDir::new()?,
    };

    // The first device learns its invitation offline; the relay lets its
    // account create spaces.
    let invitation = device::open(&dir.0, "laptop", None, false)
        .await?
        .invitation()
        .await
        .to_string();
    let relay = relay::Relay::start(&relay_bin, &dir.0, &invitation)?;
    let mut laptop = device::open(&dir.0, "laptop", Some(&relay.url), false).await?;
    let space = laptop
        .spaces()
        .create()
        .await
        .map_err(|e| e.to_string())?
        .id();
    device::synced(&laptop).await?;

    let (written, at_relay, tablet) = if args.island {
        // The second device joins the empty space and goes offline.
        let mut tablet = Second::start(&exe, &dir.0, &relay.url, "pair")?;
        tablet.pair(&laptop, space).await?;
        tablet.leave()?;
        device::synced(&laptop).await?;
        // The first device writes the burst offline, then reconnects.
        drop(laptop);
        let offline = device::open(&dir.0, "laptop", None, false).await?;
        let on = offline
            .spaces()
            .get(space)
            .await
            .map_err(|e| e.to_string())?;
        let written = import::write(&on, args.operations, args.batch).await?;
        drop((on, offline));
        let reconnected = Instant::now();
        laptop = device::open(&dir.0, "laptop", Some(&relay.url), false).await?;
        device::synced(&laptop).await?;
        let at_relay = reconnected.elapsed();
        // The second device comes back.
        (
            written,
            at_relay,
            Second::start(&exe, &dir.0, &relay.url, "resume")?,
        )
    } else {
        let on = laptop
            .spaces()
            .get(space)
            .await
            .map_err(|e| e.to_string())?;
        let start = Instant::now();
        let written = import::write(&on, args.operations, args.batch).await?;
        device::synced(&laptop).await?;
        let at_relay = start.elapsed();
        // A new device joins after the burst.
        let mut tablet = Second::start(&exe, &dir.0, &relay.url, "pair")?;
        tablet.pair(&laptop, space).await?;
        (written, at_relay, tablet)
    };
    let laptop_peak = memory::own_peak();
    let received = relay.received()?;
    let caught_up = tablet.caught_up(space, written.last)?;

    let peak = |bytes: Option<u64>| bytes.map_or("not reported".to_owned(), megabytes);
    let (scenario, from) = if args.island {
        ("both devices offline during the burst", "from reconnecting")
    } else {
        ("a new device joins after the burst", "from the first write")
    };
    println!("scenario:                 {scenario}");
    println!("operations:               {}", args.operations);
    println!("creates per change:       {}", args.batch);
    println!("changes written:          {}", written.changes);
    println!(
        "written on the device:    {}",
        seconds(written.took.as_millis())
    );
    println!(
        "held by the relay:        {} {from}",
        seconds(at_relay.as_millis())
    );
    println!(
        "first device's database:  {}",
        database(device::size(&dir.0.join("laptop")))
    );
    println!("first device's peak RSS:  {}", peak(laptop_peak));
    println!("relay log entries:        {}", received.changes);
    println!("relay frame bytes:        {}", megabytes(received.bytes));
    println!("largest frame:            {} bytes", received.largest);
    println!("frames through an upload: {}", received.large);
    println!("relay database files:     {}", megabytes(received.files));
    println!("relay peak RSS:           {}", peak(relay.peak_memory()));
    println!(
        "second device caught up:  {}",
        seconds(caught_up.millis.into())
    );
    println!("items it reads:           {}", caught_up.items);
    println!(
        "second device's database: {}",
        database((caught_up.file, caught_up.wal))
    );
    println!(
        "second device's peak RSS: {}",
        peak(Some(caught_up.peak).filter(|&b| b > 0))
    );
    if caught_up.items != args.operations {
        return Err(format!(
            "the second device reads {} items, not {}",
            caught_up.items, args.operations
        ));
    }
    Ok(())
}
