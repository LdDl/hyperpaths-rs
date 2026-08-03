//! Asserts the warm `solve_each` path is allocation-free, the property behind
//! the Graph/Workspace API. A counting global allocator tallies every
//! allocation; the test snapshots it around a warm loop and requires zero.
//!
//! Its own integration-test binary so the global counter is not polluted by
//! other tests running in parallel. Needs the `testutil` feature for the
//! synthetic-network helpers: `cargo test --features testutil --test no_alloc`.
#![cfg(feature = "testutil")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use hyperpaths_rs::Graph;
use hyperpaths_rs::testutil::{gen_grid_network, grid_full_od, grid_stops};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn solve_each_warm_is_allocation_free() {
    let (links, nodes, _, _) = gen_grid_network(8, 8, 6.0, 3.0);
    let stops = grid_stops(8, 8);
    let od = grid_full_od(&stops);

    let g = Graph::new(&links, &nodes);
    let mut w = g.new_workspace();

    // First pass grows the reusable buffers.
    w.solve_each(&od, |res| {
        black_box(res.link_vol);
    });

    // Every pass afterwards must reuse those buffers and allocate nothing.
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..50 {
        w.solve_each(&od, |res| {
            black_box(res.link_vol);
        });
    }
    let allocs = ALLOCS.load(Ordering::Relaxed) - before;

    assert_eq!(allocs, 0, "warm solve_each allocated {} times", allocs);
}
