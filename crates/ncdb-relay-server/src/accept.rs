//! Accepting the next connection. An error from `accept` is most often
//! transient: the process or the system ran out of file descriptors
//! (EMFILE, ENFILE) until connections close, or a peer gave up before the
//! server took its connection. The server then logs the error, pauses and
//! accepts again, so it neither stops nor spins while the error lasts.

use std::io;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};

/// The first pause after a failed `accept`.
const FIRST_PAUSE: Duration = Duration::from_millis(10);

/// The longest pause: each further failure in a row doubles the pause up
/// to this, so a lasting error is logged at most once a second.
const LONGEST_PAUSE: Duration = Duration::from_secs(1);

/// The next connection. Only an error that means the listener itself is
/// gone is returned; any other is logged, without content, and `accept`
/// tried again after a pause.
pub(crate) async fn next(listener: &TcpListener) -> io::Result<TcpStream> {
    let mut pause = FIRST_PAUSE;
    loop {
        match listener.accept().await {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if listener_gone(&error) => return Err(error),
            Err(error) => {
                eprintln!(
                    "ncdb-relay: accept failed, trying again in {} ms: {error}",
                    pause.as_millis()
                );
                tokio::time::sleep(pause).await;
                pause = (pause * 2).min(LONGEST_PAUSE);
            }
        }
    }
}

/// Whether `error` means the listener itself is gone. None does yet.
fn listener_gone(_error: &io::Error) -> bool {
    false
}
