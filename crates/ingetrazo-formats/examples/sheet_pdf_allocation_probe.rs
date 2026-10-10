use ingetrazo_formats::sheet_pdf::{encode, Draw, Page, PathOp};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};
struct Alloc;
static TRACK: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
        System.realloc(p, l, n)
    }
}
#[global_allocator]
static ALLOC: Alloc = Alloc;
fn main() {
    let mut path = vec![PathOp::Move([-0., 0.0000000000005])];
    for i in 0..50000 {
        let x = (i % 100) as f64 + 1.234567890123;
        path.push(PathOp::Cubic([
            x,
            2.345678901234,
            x + 0.01,
            3.456789012345,
            x + 0.02,
            4.567890123456,
        ]));
    }
    let pages = vec![Page {
        device_dpi: Some(300.),
        size_mm: [210., 297.],
        media_box_points: Some([595., 842.]),
        origin_mm: [42. * 25.4 / 300.; 2],
        draws: vec![Draw {
            origin_mm: [-0., 5.678901234567],
            even_odd: true,
            path,
            stroke: Some(([0.1, 0.2, 0.3, 0.4], 0.35)),
            fill: None,
            stroke_options: None,
            clip: None,
            clip_intersection: None,
        }],
    }];
    TRACK.store(true, Ordering::Relaxed);
    let bytes = encode(&pages).unwrap();
    TRACK.store(false, Ordering::Relaxed);
    let allocations = CALLS.load(Ordering::Relaxed);
    let destination = std::env::args().nth(1).unwrap();
    std::fs::write(&destination, &bytes).unwrap();
    let mut timings = Vec::new();
    for _ in 0..9 {
        let start = Instant::now();
        let output = encode(&pages).unwrap();
        std::hint::black_box(output);
        timings.push(start.elapsed().as_nanos() as u64);
    }
    println!(
        "{}",
        serde_json::json!({"commands":50001,"numeric_coordinates":300002,"bytes":bytes.len(),"allocation_calls_including_realloc":allocations,"encode_nanoseconds":timings,"scope":"Release in-process vector encode only, no prepare/IO timing; instrumented allocator counts separately from repeated timing, no renderer/full application claim."})
    );
}
