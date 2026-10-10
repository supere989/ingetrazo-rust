#[cfg(target_os = "macos")]
fn main() -> Result<(), String> {
    use std::time::Duration;
    let metal = ingetrazo_metal::NativeMetal::new()?;
    println!(
        "Native Metal submission device {}; unified {}; runner hardware must be classified separately",
        metal.info.name, metal.info.unified_memory
    );
    let bytes = 17 * 1024 * 1024;
    let before = metal.budget.lock().unwrap().total_committed();
    assert!(metal.fill_readback(3, 157).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
    let mut work = metal.fill_readback(bytes, 157)?;
    assert!(work.verify_readback(157).is_err());
    assert!(metal.budget.lock().unwrap().total_committed() >= before + 2 * bytes as u64);
    assert!(
        work.wait(Duration::from_secs(5))?,
        "GPU completion deadline expired"
    );
    assert_eq!(work.verify_readback(157)?, bytes);
    drop(work);
    assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
    // Actual ledger admission failure after the first resource allocation must
    // drop the unsubmitted private owner, preserving prior allocation accounting.
    let sample = metal.buffer(bytes, false)?;
    let private_charge = sample.allocation_bytes();
    drop(sample);
    metal.budget.lock().unwrap().total_limit = Some(before + private_charge);
    assert!(metal.fill_readback(bytes, 37).is_err());
    assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
    metal.budget.lock().unwrap().total_limit = None;
    let mut pending = metal.fill_readback(bytes, 93)?;
    assert!(!pending.wait(Duration::ZERO)?);
    let charged = metal.budget.lock().unwrap().total_committed();
    assert!(charged > before);
    assert!(metal.buffer(4, true).is_err());
    assert!(pending.verify_readback(93).is_err());
    assert!(pending.wait(Duration::from_secs(5))?);
    assert_eq!(pending.verify_readback(93)?, bytes);
    drop(pending);
    assert_eq!(metal.budget.lock().unwrap().total_committed(), before);
    // Separate session: unknown Drop must not falsely remove accounting, even if
    // GPU happened to finish before Drop. No completion observation was made.
    let unknown = ingetrazo_metal::NativeMetal::new()?;
    let mut pending = unknown.fill_readback(4096, 37)?;
    assert!(!pending.wait(Duration::ZERO)?);
    let ledger = unknown.budget.clone();
    let retained = ledger.lock().unwrap().total_committed();
    drop(pending);
    drop(unknown);
    assert_eq!(ledger.lock().unwrap().total_committed(), retained);
    println!(
        "PASS native Metal GPU fill/17MiB BLIT/readback every byte; actual completion retirement; partial admission rollback; zero-deadline unknown retention and poison; unknown Drop ledger retained. Zero-deadline control is not driver-stall proof. Rendering/physical hardware/RT remain OPEN"
    );
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal requires macOS; no fallback");
    std::process::exit(2);
}
