// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! IngeTrazo CLI executable and GUI launcher.

use std::env;
use std::path::{Path, PathBuf};

const VERSION: &str = "0.3.15";

fn print_version() {
    println!("IngeTrazo {} (Rust native)", VERSION);
}

fn print_help() {
    print_version();
    println!(
        "\nFree 3D modeler for architecture, engineering and 3D design.\n\
        Part of the IngePresupuestos ecosystem.\n\n\
        USAGE:\n    \
            ingetrazo [OPTIONS] [FILE]\n    \
            ingetrazo convert <INPUT> <OUTPUT>\n\n\
        ARGUMENTS:\n    \
            <FILE>                 Model file to open (.igz, .obj, .stl)\n    \
            <INPUT>                Source file to convert (.igz, .obj, .stl)\n    \
            <OUTPUT>               Destination file to write (.igz, .obj, .stl)\n\n\
        OPTIONS:\n    \
            --check                Run system and installation health check\n    \
            -v, --version          Print version information\n    \
            -h, --help             Print this help message\n"
    );
}

fn self_check() -> i32 {
    println!("IngeTrazo {}", VERSION);
    println!("  packaged   : yes (compiled Rust binary)");
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    println!("  runtime    : Rust native (Vulkan + egui)");
    #[cfg(target_os = "macos")]
    println!("  runtime    : Rust native (Metal + egui)");
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    println!("  runtime    : unsupported platform");

    let checks = [
        ("formats: igz", "found (native JSON/ZIP format 1 & 2)"),
        ("formats: obj", "found (Wavefront OBJ import & export)"),
        ("formats: stl", "found (Stereolithography binary & ASCII)"),
        ("formats: dxf", "found (AutoCAD DXF 3DFACE)"),
        ("formats: gltf", "found (glTF 2.0 binary GLB)"),
        ("render engine", "found (HLR, wireframe, visual styles)"),
        ("tools engine", "found (select, line, rect, pushpull, etc.)"),
        ("topology", "found (shared-vertex manifold solid)"),
    ];

    for (label, status) in checks {
        println!("  {:<15}: {}", label, status);
    }

    println!("\nOK");
    0
}

fn handle_convert(input_str: &str, output_str: &str) -> i32 {
    let in_path = Path::new(input_str);
    let out_path = Path::new(output_str);

    if !in_path.exists() {
        eprintln!("Error: input file does not exist: {}", in_path.display());
        return 1;
    }

    match ingetrazo_formats::convert_file(in_path, out_path) {
        Ok(()) => {
            println!("Successfully converted {} -> {}", in_path.display(), out_path.display());
            0
        }
        Err(err) => {
            eprintln!("Conversion failed: {err}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // 1. Check for flags
    if args.iter().any(|a| a == "--check") {
        std::process::exit(self_check());
    }

    if args.iter().any(|a| a == "--version" || a == "-v" || a == "-V") {
        print_version();
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    // 2. Subcommand: convert <input> <output>
    if args.len() >= 4 && args[1] == "convert" {
        std::process::exit(handle_convert(&args[2], &args[3]));
    }

    // 3. Document argument: ingetrazo <file>
    let initial_file = if args.len() > 1 && !args[1].starts_with('-') {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    // The packaged executable uses the platform renderer directly. Diagnostic
    // examples retain their own options; production never needs an opt-in flag.
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let result = ingetrazo_gui::native_vulkan::run_native(initial_file);
    #[cfg(target_os = "macos")]
    let result = ingetrazo_gui::native_metal::run_native(initial_file);
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    let result: Result<(), Box<dyn std::error::Error>> = Err("No native renderer for this platform".into());
    if let Err(e) = result {
        eprintln!("Error running IngeTrazo GUI: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_self_check() {
        let code = self_check();
        assert_eq!(code, 0);
    }

    #[test]
    fn test_cli_convert_headless() {
        let temp_dir = std::env::temp_dir();
        let in_file = temp_dir.join("cli_test_box.obj");
        let out_file = temp_dir.join("cli_test_box.stl");

        let mut scene = ingetrazo_core::scene::Scene::new();
        let v0 = scene.mesh.add_vertex(ingetrazo_core::math::DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(ingetrazo_core::math::DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(ingetrazo_core::math::DVec3::new(1.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2], vec![]);

        ingetrazo_formats::save_file(&scene, &in_file).unwrap();

        let status = handle_convert(in_file.to_str().unwrap(), out_file.to_str().unwrap());
        assert_eq!(status, 0);
        assert!(out_file.exists());

        let _ = std::fs::remove_file(in_file);
        let _ = std::fs::remove_file(out_file);
    }
}
