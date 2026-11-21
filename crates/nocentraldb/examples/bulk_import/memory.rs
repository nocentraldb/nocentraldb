//! Peak resident memory, read from `/proc/<pid>/status` (`VmHWM`). Where
//! the platform has no such file, the measurement reports none.

/// The peak resident memory the status file at `path` reports, in bytes.
pub fn peak(path: &str) -> Option<u64> {
    let status = std::fs::read_to_string(path).ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// This process's peak resident memory, in bytes.
pub fn own_peak() -> Option<u64> {
    peak("/proc/self/status")
}
