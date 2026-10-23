//! REQ-STREAM-020 natively: appending 3,000,000 rows to a chart whose ring
//! holds 1,000,000 grows no memory after the first 1,000,000.
//!
//! A test binary of its own, because it counts every allocation through a
//! global allocator: the live bytes and their peak. The chart is a `line`
//! chart with the default `retain.rows` (1,000,000), given rows by `append`
//! in batches of 1,000 as the boundary builds them (`Table::staged` from a
//! staging buffer of two columns), with an 80 × 24 frame after every
//! hundredth batch. After the first 1,000,000 rows the peak is reset to the
//! live bytes; after 2,000,000 more, the peak may stand above that mark by
//! no more than one batch and one frame's scratch (256 KiB here), where
//! one reallocation of the ring's two columns would be 16 MB.
//!
//! The Node test (`web/test/memory.test.mjs`) checks the same through the
//! module: `memory.buffer.byteLength` read after the first 1,000,000 rows
//! (streamed through `staging(n)` and `append(n)` with frames between)
//! equals the byteLength after 3,000,000, since wasm memory grows only when
//! the allocator runs out (`memory.grow`), and never shrinks.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use glyphplot::chart::Env;
use glyphplot::data::{Column, Data, Role, Table};
use glyphplot::grid::Grid;
use glyphplot::registry;

/// The system allocator, counting the bytes live and their peak.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is passed to `System` unchanged; the counters only add
// and subtract the sizes it was asked for.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            let live = LIVE.fetch_add(new_size, Ordering::Relaxed) + new_size;
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// The rows a batch.
const BATCH: usize = 1_000;

/// REQ-STREAM-020, ADR-107.
#[test]
fn the_ring_grows_no_memory_past_its_rows() {
    let template = Table::from_columns(vec![Column::f64("x", Vec::new()).role(Role::X), Column::f64("y", Vec::new()).role(Role::Y)]).unwrap();
    let (_, mut chart) = registry::make("line", "{}").expect("a line chart");
    let mut staging = vec![0.0; 2 * BATCH];
    let mut grid = Grid::new(80, 24);
    let mut batch = |chart: &mut Box<dyn glyphplot::Chart>, from: usize, grid: &mut Grid| {
        for i in 0..BATCH {
            let x = (from + i) as f64;
            staging[i] = x;
            staging[BATCH + i] = (x / 997.0).sin();
        }
        let rows = template.staged(&staging, BATCH).expect("staged rows");
        chart.append(Data::from_table(rows)).expect("appended");
        if (from / BATCH) % 100 == 99 {
            chart.frame(0.0, &Env::plain(), grid);
        }
    };
    let mut at = 0;
    while at < 1_000_000 {
        batch(&mut chart, at, &mut grid);
        at += BATCH;
    }
    chart.frame(0.0, &Env::plain(), &mut grid);
    let mark = LIVE.load(Ordering::Relaxed);
    PEAK.store(mark, Ordering::Relaxed);
    while at < 3_000_000 {
        batch(&mut chart, at, &mut grid);
        at += BATCH;
    }
    chart.frame(0.0, &Env::plain(), &mut grid);
    let (peak, live) = (PEAK.load(Ordering::Relaxed), LIVE.load(Ordering::Relaxed));
    println!("stream memory: {mark} B live after 1,000,000 rows; peak {peak} B and {live} B live after 3,000,000");
    assert!(mark > 16_000_000, "the ring holds two columns of 1,000,000 rows: {mark} B");
    assert!(peak <= mark + 256 * 1024, "the peak grew {} B past the mark of {mark} B", peak - mark);
    assert!(live <= mark + 4096, "{} B more live after 3,000,000 rows", live.saturating_sub(mark));
}
