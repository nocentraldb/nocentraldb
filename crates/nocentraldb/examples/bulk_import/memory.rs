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

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

/// The bytes this process holds on Rust's heap now.
static LIVE: AtomicIsize = AtomicIsize::new(0);

/// The system allocator, counting the bytes the process holds on Rust's
/// heap (NOC-452). SQLite allocates outside it.
pub struct Counting;

// SAFETY: every call goes on to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(
            new_size as isize - layout.size() as isize,
            Ordering::Relaxed,
        );
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// The bytes the process holds on Rust's heap now.
pub fn heap() -> u64 {
    LIVE.load(Ordering::Relaxed).max(0) as u64
}
