//! The peak heap of one piece of code, counted by a global allocator that
//! a test binary installs: `#[global_allocator] static A: Counting =
//! Counting;`. Counts are per thread, so tests that run side by side on
//! other threads do not disturb each other.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static LIVE: Cell<isize> = const { Cell::new(0) };
    static PEAK: Cell<isize> = const { Cell::new(0) };
}

/// The system allocator, counting this thread's live bytes and their peak.
pub struct Counting;

fn count(delta: isize) {
    // `try_with`: a thread may allocate while its locals are torn down.
    let _ = LIVE.try_with(|live| {
        let now = live.get() + delta;
        live.set(now);
        let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
    });
}

// SAFETY: every call goes on to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count(-(layout.size() as isize));
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size as isize - layout.size() as isize);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// What one run held: the most heap at any point, and what was still
/// held when it returned, both above what was live when it started, in
/// bytes. What the run returns counts in both.
pub struct Held {
    pub peak: usize,
    pub kept: usize,
}

impl Held {
    /// What the run held at its peak beyond what it returned or kept.
    pub fn transient(&self) -> usize {
        self.peak - self.kept
    }
}

/// What `run` returns, and what it held.
pub fn peak<T>(run: impl FnOnce() -> T) -> (T, Held) {
    let start = LIVE.with(Cell::get);
    PEAK.with(|peak| peak.set(start));
    let out = run();
    let top = PEAK.with(Cell::get);
    let end = LIVE.with(Cell::get);
    let held = Held {
        peak: (top - start).max(0) as usize,
        kept: (end - start).max(0) as usize,
    };
    (out, held)
}
