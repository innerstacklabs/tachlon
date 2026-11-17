//! Profile builds only (cargo feature `alloc-stats`, which `scripts/build-web.sh --keep-names` turns on): a counting
//! global allocator, read through `App.test_alloc_stats`. The shipped build has neither.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

pub(super) static LIVE: AtomicUsize = AtomicUsize::new(0);
pub(super) static PEAK: AtomicUsize = AtomicUsize::new(0);
pub(super) static TOTAL: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: every call forwards unchanged to the system allocator; the counters only observe it.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
        PEAK.fetch_max(live, Relaxed);
        TOTAL.fetch_add(1, Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        LIVE.fetch_sub(layout.size(), Relaxed);
        let live = LIVE.fetch_add(new, Relaxed).wrapping_add(new);
        PEAK.fetch_max(live, Relaxed);
        TOTAL.fetch_add(1, Relaxed);
        unsafe { System.realloc(ptr, layout, new) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;
