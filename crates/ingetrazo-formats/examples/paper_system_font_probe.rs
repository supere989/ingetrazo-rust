// SPDX-License-Identifier: GPL-3.0-or-later
//! Observe the production resolver against a retained actual native Qt witness.
#[cfg(target_os = "macos")]
fn main() {
    use ingetrazo_formats::paper_font_resolver::{Request, Resolver};
    let mut resolver = Resolver::new().expect("native provider");
    let request = Request {
        family: "Sans Serif".into(),
        bold: false,
        italic: false,
        required_chars: vec![],
    };
    let matched = resolver
        .resolve(request.clone())
        .expect("source default request");
    let stats = resolver.stats();
    let again = resolver.resolve(request).expect("cached request");
    assert!(std::rc::Rc::ptr_eq(&matched.font, &again.font));
    assert_eq!(stats, resolver.stats());
    let bytes = matched.font.shared_bytes();
    let face =
        ttf_parser::Face::parse(bytes, matched.font.key().face_index).expect("owned metadata");
    let mut tables = serde_json::Map::new();
    let mut total = 0;
    for tag in [
        *b"head", *b"hhea", *b"maxp", *b"OS/2", *b"name", *b"cmap", *b"hmtx",
    ] {
        let table = face
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(&tag))
            .expect("identity table");
        total += table.len();
        assert!(total <= 256 * 1024, "bounded metadata observation");
        tables.insert(
            String::from_utf8(tag.to_vec()).unwrap(),
            serde_json::Value::String(table.iter().map(|v| format!("{v:02x}")).collect()),
        );
    }
    println!(
        "{}",
        serde_json::json!({"path":matched.path,"index":matched.index,
        "ascent":matched.font.ascent_26_6() as f64/64.,"descent":matched.font.descent_26_6() as f64/64.,
        "warm_same_owner_no_matching":true,"table_hex":tables})
    );
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native CoreText observation requires macOS");
    std::process::exit(2);
}
