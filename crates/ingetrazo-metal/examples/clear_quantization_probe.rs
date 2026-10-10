//! Native BGRA8 GPU target readback, separate from WindowServer/ICC capture.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use ingetrazo_metal::{egui_painter::EguiPainter, NativeMetal};
    use std::{path::PathBuf, time::Duration};
    let root = std::env::args()
        .find_map(|a| a.strip_prefix("--output=").map(PathBuf::from))
        .ok_or("--output required")?;
    std::fs::create_dir_all(&root)?;
    if std::fs::read_dir(&root)?.next().is_some() {
        return Err("Fresh diagnostic output required".into());
    }
    let metal = NativeMetal::new()?;
    let mut painter = EguiPainter::new(&metal)?;
    let paint = painter.prepare(&metal, &[], 1.)?;
    let mut results = Vec::new();
    for (name, color) in [
        ("actual-fractional-clear", [0.08, 0.2, 0.35, 1.]),
        ("exact-byte-clear", [20. / 255., 51. / 255., 89. / 255., 1.]),
        ("black-control", [0., 0., 0., 1.]),
        ("white-control", [1.; 4]),
    ] {
        let mut frame =
            painter.submit_clear_diagnostic(&metal, paint.clone(), 64, 64, false, color)?;
        let bytes = frame.wait_readback(Duration::from_secs(5))?;
        let pixel = [bytes[0], bytes[1], bytes[2], bytes[3]];
        assert_eq!(bytes.len(), 64 * 64 * 4);
        assert!(
            bytes.chunks_exact(4).all(|p| p == pixel),
            "Actual GPU clear must be uniform"
        );
        if name == "exact-byte-clear" {
            assert_eq!(pixel, [20, 51, 89, 255]);
        }
        if name == "black-control" {
            assert_eq!(pixel, [0, 0, 0, 255]);
        }
        if name == "white-control" {
            assert_eq!(pixel, [255; 4]);
        }
        std::fs::write(root.join(format!("{name}.rgba")), &bytes)?;
        results.push(format!(
            "{{\"name\":{name:?},\"clear\":{color:?},\"raw_gpu_rgba\":{pixel:?}}}"
        ));
    }
    assert!(painter
        .submit_clear_diagnostic(&metal, paint.clone(), 64, 64, false, [f64::NAN, 0., 0., 1.])
        .is_err());
    drop((paint, painter));
    assert_eq!(metal.budget.lock().unwrap().total_committed(), 0);
    std::fs::write(root.join("result.json"),format!("{{\"passed\":true,\"device\":{:?},\"unified\":{},\"target_format\":\"BGRA8Unorm\",\"gpu_to_host_readback\":true,\"windowserver_color_processing\":false,\"drawable_readback_claim\":false,\"cases\":[{}],\"invalid_clear_negative\":true,\"final_ledger\":0}}",metal.info.name,metal.info.unified_memory,results.join(",")))?;
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Actual native Metal required");
    std::process::exit(2);
}
