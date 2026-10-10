use ingetrazo_formats::paper_font_resolver::{Request, Resolver};
fn main() {
    let out = std::env::args().nth(1).unwrap();
    let mut resolver = Resolver::new().unwrap();
    let mut requests = Vec::new();
    let mut paths = std::collections::HashSet::new();
    for family in [
        "DejaVu Sans",
        "DejaVu Sans Mono",
        "DejaVu Serif",
        "Droid Sans Fallback",
        "Noto Sans Mono",
        "Noto Mono",
        "Liberation Sans",
        "Liberation Serif",
        "Liberation Mono",
    ] {
        for bold in [false, true] {
            for italic in [false, true] {
                let request = Request {
                    family: family.into(),
                    bold,
                    italic,
                    required_chars: vec![],
                };
                let m = resolver.resolve(request.clone()).unwrap();
                if paths.insert((m.path, m.index)) {
                    requests.push(request);
                }
            }
        }
    }
    assert!(
        paths.len() > 16,
        "Actual distinct fonts must evict the sixteen-face resource cache"
    );
    let retained = resolver.resolve(requests[0].clone()).unwrap();
    let retained_key = retained.font.key();
    for _ in 0..12 {
        for request in &requests {
            let _ = resolver.resolve(request.clone()).unwrap();
        }
    }
    for i in 0..80 {
        let _ = resolver
            .resolve(Request {
                family: format!("NotInstalledFont-{i}"),
                bold: false,
                italic: false,
                required_chars: vec![],
            })
            .unwrap();
    }
    let returned = resolver.resolve(requests[0].clone()).unwrap();
    assert_eq!(
        retained_key,
        returned.font.key(),
        "Live physical font identity survives FIFO and face cache retirement"
    );
    let (entries, queue) = resolver.request_cache_entries();
    let stats = resolver.stats();
    let report = serde_json::json!({"actual_distinct_font_files":paths.len(),"request_entries":entries,"fifo_entries":queue,"cache_fonts":stats.2.0,"cold_matches":stats.1,"passed":entries==queue&&queue<=64});
    let old_key = returned.font.key();
    resolver.refresh().unwrap();
    let refreshed = resolver.resolve(requests[0].clone()).unwrap();
    assert_ne!(
        old_key,
        refreshed.font.key(),
        "Successful refresh must change font epoch"
    );
    assert!(!retained
        .font
        .shape_run("retained", None)
        .unwrap()
        .is_empty());
    std::fs::write(out, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    assert_eq!(
        entries, queue,
        "Expired weak entry replacement must keep FIFO unique"
    );
    assert!(queue <= 64);
    println!("PASS real font-cache eviction keeps request FIFO bounded");
}
