#[cfg(target_os = "macos")]
fn main() -> Result<(), String> {
    let metal = ingetrazo_metal::NativeMetal::new()?;
    println!("Native Metal device {}; unified {}; working set {}; max buffer {}; RT advertised {}; BC advertised {}", metal.info.name, metal.info.unified_memory, metal.info.recommended_working_set, metal.info.max_buffer_length, metal.info.ray_tracing_advertised, metal.info.bc_compression_advertised);
    let before = metal.budget.lock().unwrap().total_committed();
    let mut buffer = metal.buffer(17 * 1024 * 1024, true)?;
    buffer.write(&[37, 93, 157, 128])?;
    assert!(buffer.allocation_bytes() >= buffer.length() as u64);
    drop(buffer);
    assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
    println!("Native Metal device and buffer allocation controls PASS; GPU submission, rendering and hardware parity remain unproven");
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal requires macOS; no fallback backend selected");
    std::process::exit(2);
}
