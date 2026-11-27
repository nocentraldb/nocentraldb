//! Opening a space with many persons holds memory that grows with the
//! persons, not with their square (NOC-442). a consumer using NoCentralDB measured a tab
//! above 4 GB at 10,000 persons: each node kept its causal past as a set
//! of change ids, about 2 GB at 10,000 persons. This test binary counts
//! every allocation, on both targets, so it holds this one test only.

#[path = "support/persons.rs"]
mod persons;
#[path = "support/space.rs"]
mod space;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use ncdb_core::membership::Resolver;

/// The system allocator, counting the bytes allocated now and the most
/// allocated at once since the last [`Counting::reset`].
struct Counting {
    now: AtomicUsize,
    peak: AtomicUsize,
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract for `alloc` is passed on unchanged.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let now = self.now.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            self.peak.fetch_max(now, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract for `dealloc` is passed on unchanged.
        unsafe { System.dealloc(ptr, layout) };
        self.now.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

impl Counting {
    /// Starts a new peak from the bytes allocated now, and returns them.
    fn reset(&self) -> usize {
        let now = self.now.load(Ordering::Relaxed);
        self.peak.store(now, Ordering::Relaxed);
        now
    }
}

#[global_allocator]
static ALLOCATED: Counting = Counting {
    now: AtomicUsize::new(0),
    peak: AtomicUsize::new(0),
};

/// The most a resolve of 10,000 persons may allocate at once on top of the
/// changes it is given: about 13 KB per person. It allocates about 7 KB
/// per person: each body with its dependencies, kept once and read once,
/// and the person keys of the fold.
const BOUND: usize = 128 << 20;

/// The most a resolve allocates at once, on top of the changes it is
/// given, for a space of `persons` persons.
fn peak(persons: u32) -> usize {
    let (space, changes) = persons::chain(persons);
    let before = ALLOCATED.reset();
    let mut resolver = Resolver::new(space.space);
    let resolved = resolver.resolve(&changes);
    let peak = ALLOCATED.peak.load(Ordering::Relaxed) - before;
    assert_eq!(resolved.person_keys().count(), persons as usize);
    peak
}

#[test]
fn opening_a_space_of_10k_persons_stays_within_128_mib_and_grows_with_the_persons() {
    let quarter = peak(2_500);
    let all = peak(10_000);
    assert!(all <= BOUND, "{} MiB at once", all >> 20);
    // Four times the persons: four times the memory, sixteen times if it
    // grew with their square.
    assert!(
        all <= 5 * quarter,
        "{} KiB for 2,500 persons, {} KiB for 10,000",
        quarter >> 10,
        all >> 10
    );
}
