#!/usr/bin/env python3
"""Rebuild checked-in SPIR-V with pinned shaderc, outside runtime dependencies."""
from pathlib import Path
import subprocess,tempfile
root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="ingetrazo-shader-") as temporary:
 tool=Path(temporary);(tool/"src").mkdir()
 (tool/"Cargo.toml").write_text('[package]\nname="ingetrazo-shader-build"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nshaderc="=0.8.3"\n')
 (tool/"src/main.rs").write_text('fn main(){let a=std::env::args().collect::<Vec<_>>();let source=std::fs::read_to_string(&a[1]).unwrap();let kind=if a[1].ends_with(".vert"){shaderc::ShaderKind::Vertex}else{shaderc::ShaderKind::Fragment};let compiler=shaderc::Compiler::new().unwrap();let compiled=compiler.compile_into_spirv(&source,kind,&a[1],"main",None).unwrap();std::fs::write(&a[2],compiled.as_binary_u8()).unwrap();}\n')
 for name in ["world.vert","world.frag", "transparent.frag","contour.vert"]:
  source=root/"crates/ingetrazo-gui/src/native_vulkan/shaders"/name
  subprocess.run(["cargo","run","--quiet","--manifest-path",str(tool/"Cargo.toml"),"--",str(source),str(source)+".spv"],check=True)

 # The encoded opaque viewport image uses a per-texture fragment push flag.
 source=root/"vendor/egui-ash-renderer/src/shaders/shader.frag"
 subprocess.run(["cargo","run","--quiet","--manifest-path",str(tool/"Cargo.toml"),"--",str(source),str(source)+".spv"],check=True)
