fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        let source = std::path::PathBuf::from(std::env::args().nth(1).ok_or("SOURCE required")?);
        let output = std::path::PathBuf::from(std::env::args().nth(2).ok_or("OUTPUT required")?);
        ingetrazo_gui::native_metal::run_bridge_capture_probe(source, output)?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Actual native Metal macOS host required".into())
    }
}
