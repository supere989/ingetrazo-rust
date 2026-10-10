use ingetrazo_formats::paper_font_resolver::{Request, Resolver};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut resolver = Resolver::new().unwrap();
    let mut rows = Vec::new();
    for case in source["cases"].as_array().unwrap() {
        let call = &case["source_draw_calls"][0];
        let s = &call["font"];
        let request = Request {
            family: s["requested"].as_str().unwrap().into(),
            bold: s["bold"].as_bool().unwrap(),
            italic: s["italic"].as_bool().unwrap(),
            required_chars: Vec::new(),
        };
        let result = resolver.resolve(request.clone()).unwrap();
        let before = resolver.stats();
        let repeat = resolver.resolve(request).unwrap();
        assert!(std::rc::Rc::ptr_eq(&result.font, &repeat.font));
        assert_eq!(before, resolver.stats());
        assert_eq!(result.path.to_str().unwrap(), s["file"].as_str().unwrap());
        assert_eq!(
            result.font.ascent_26_6() as f64 / 64.,
            s["ascent"].as_f64().unwrap()
        );
        assert_eq!(
            result.font.descent_26_6() as f64 / 64.,
            s["descent"].as_f64().unwrap()
        );
        rows.push(serde_json::json!({"case":case["name"],"path":result.path,"face_index":result.index,"hinting":format!("{:?}",result.policy),"warm_no_match_or_load":true}));
    }
    let before = resolver.stats();
    assert!(resolver
        .resolve(Request {
            family: "bad\0name".into(),
            bold: false,
            italic: false,
            required_chars: vec![]
        })
        .is_err());
    assert_eq!(before, resolver.stats());
    let retained = resolver
        .resolve(Request {
            family: "Sans Serif".into(),
            bold: false,
            italic: false,
            required_chars: vec![],
        })
        .unwrap();
    resolver.refresh().unwrap();
    assert_eq!(resolver.stats().0, 0);
    assert!(!retained
        .font
        .shape_run("retained", None)
        .unwrap()
        .is_empty());
    let report = serde_json::json!({"passed":true,"cases":rows,"scope":"Linux native Fontconfig family/style/index/hint-policy against nineteen retained actual helper records. No paragraph fallback segmentation or Mac/Windows provider claim.","negative":"NUL family refuses without cache mutation","refresh":"Explicit epoch reset keeps outstanding font owner valid"});
    std::fs::write(&args[2], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("PASS {} actual font request controls", rows.len());
}
