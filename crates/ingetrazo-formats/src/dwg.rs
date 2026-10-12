//! LibreDWG satellite bridge. No DWG parser or Python runtime.
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub const MAX_DXF_BYTES: usize = 256 * 1024 * 1024;
const MAX_LOG: usize = 1024 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(300);
/// Actual original discovery order, including its development sibling courtesy.
pub fn find_tool(root: &Path, name: &str) -> Option<PathBuf> {
    find_tool_for_platform(root, name, cfg!(windows))
}
/// Deterministic packaging admission; production delegates its actual platform.
pub fn find_tool_for_platform(root: &Path, name: &str, windows: bool) -> Option<PathBuf> {
    let exe = if windows {
        format!("{name}.exe")
    } else {
        name.into()
    };
    let sibling = root
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("ingecad/vendor/libredwg/bin"));
    for base in [Some(root.join("vendor/libredwg/bin")), sibling]
        .into_iter()
        .flatten()
    {
        let p = base.join(&exe);
        if p.is_file() {
            return Some(p);
        }
    }
    for base in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let p = base.join(&exe);
        if p.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if p.metadata().ok()?.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            return Some(p);
        }
    }
    None
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn temp() -> Result<Temp, String> {
    let base = std::env::temp_dir();
    if !base.to_string_lossy().is_ascii() {
        return Err("ASCII-only conversion temporary directory unavailable".into());
    }
    for _ in 0..16 {
        let p = base.join(format!("ingetrazo-dwg-{}", uuid::Uuid::new_v4()));
        match std::fs::create_dir(&p) {
            Ok(()) => return Ok(Temp(p)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Conversion temporary directory collision".into())
}
fn diagnostic(path: &Path) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_LOG as u64)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}
pub struct Converted {
    pub bytes: Vec<u8>,
    pub warnings: String,
    pub status: Option<i32>,
    pub null_handles: usize,
    pub duplicate_handles: usize,
}
/// Output presence, rather than exit code, is the source verdict; DXF admission follows later.
pub fn convert(tool: &Path, input: &Path, timeout: Duration) -> Result<Converted, String> {
    let dir = temp()?;
    let output = dir.0.join("converted.dxf");
    // File-backed diagnostics cannot fill a pipe or hold timeout cleanup hostage.
    let stdout_path = dir.0.join("stdout.log");
    let stderr_path = dir.0.join("stderr.log");
    let stdout_file = std::fs::File::create(&stdout_path).map_err(|e| e.to_string())?;
    let stderr_file = std::fs::File::create(&stderr_path).map_err(|e| e.to_string())?;
    let mut child = Command::new(tool)
        .args(["-y", "-o"])
        .arg(&output)
        .arg(input)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    let verdict = loop {
        if [&stdout_path, &stderr_path]
            .iter()
            .any(|p| std::fs::metadata(p).is_ok_and(|m| m.len() > MAX_LOG as u64))
            || std::fs::metadata(&output).is_ok_and(|m| m.len() > MAX_DXF_BYTES as u64)
        {
            let _ = child.kill();
            let _ = child.wait();
            break Err("Converter output budget exceeded".to_string());
        }
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("converter timed out".to_string());
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    if [&stdout_path, &stderr_path]
        .iter()
        .any(|p| std::fs::metadata(p).is_ok_and(|m| m.len() > MAX_LOG as u64))
    {
        return Err("Converter diagnostic budget exceeded".into());
    }
    let stdout = diagnostic(&stdout_path)?;
    let stderr = diagnostic(&stderr_path)?;
    let status = verdict?;
    let file = std::fs::File::open(&output);
    let mut bytes = Vec::new();
    if let Ok(f) = file {
        if f.metadata().map_err(|e| e.to_string())?.is_file() {
            f.take((MAX_DXF_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
        }
    }
    if bytes.len() > MAX_DXF_BYTES {
        return Err("Converted DXF exceeds 256 MiB".into());
    }
    if bytes.is_empty() {
        let log = if stderr.is_empty() { &stdout } else { &stderr };
        let text = String::from_utf8_lossy(log);
        let lines = text.trim().lines().collect::<Vec<_>>();
        let tail = lines[lines.len().saturating_sub(3)..].join(" | ");
        return Err(format!(
            "conversion produced no output: {}",
            if tail.is_empty() {
                format!("rc={}", status.code().unwrap_or(-1))
            } else {
                tail
            }
        ));
    }
    let (bytes, null_handles, duplicate_handles) = repair(&bytes)?;
    Ok(Converted {
        bytes,
        warnings: String::from_utf8_lossy(&stderr).into_owned(),
        status: status.code(),
        null_handles,
        duplicate_handles,
    })
}
fn trim(b: &[u8]) -> &[u8] {
    let ws = |c: u8| matches!(c,9..=13|28..=32|85|160);
    let start = b.iter().position(|c| !ws(*c)).unwrap_or(b.len());
    let end = b.iter().rposition(|c| !ws(*c)).map_or(start, |i| i + 1);
    &b[start..end]
}
/// Only literal LF separates pairs. All untouched Latin-1/control bytes survive exactly.
pub fn repair(bytes: &[u8]) -> Result<(Vec<u8>, usize, usize), String> {
    if bytes.len() > MAX_DXF_BYTES {
        return Err("DXF repair byte limit exceeded".into());
    }
    let count = bytes.iter().filter(|c| **c == b'\n').count() + 1;
    // Retained input, split owners, repaired owners, and output must fit cold budget.
    let charge = bytes
        .len()
        .checked_mul(5)
        .and_then(|n| count.checked_mul(192).and_then(|m| n.checked_add(m)))
        .ok_or("DXF repair budget overflow")?;
    if charge > 512 * 1024 * 1024 {
        return Err("DXF repair working budget exceeded".into());
    }
    let lines = bytes.split(|c| *c == b'\n').collect::<Vec<_>>();
    if lines.len() > 8_000_000 {
        return Err("DXF repair pair limit exceeded".into());
    }
    let mut out: Vec<Vec<u8>> = Vec::new();
    out.try_reserve(lines.len())
        .map_err(|_| "DXF repair allocation refused")?;
    let mut nulls = 0;
    let mut i = 0;
    while i + 1 < lines.len() {
        if trim(lines[i]) == b"5" && trim(lines[i + 1]) == b"0" {
            nulls += 1;
        } else {
            out.push(lines[i].to_vec());
            out.push(lines[i + 1].to_vec());
        }
        i += 2;
    }
    for l in &lines[i..] {
        out.push(l.to_vec())
    }
    let mut spots = Vec::new();
    let mut seed = None;
    let mut section: Option<Vec<u8>> = None;
    let mut expecting = false;
    i = 0;
    while i + 1 < out.len() {
        let code = trim(&out[i]);
        let value = trim(&out[i + 1]);
        if code == b"0" {
            if value == b"ENDSEC" {
                section = None
            }
            expecting = ![b"SECTION".as_slice(), b"ENDSEC", b"EOF"].contains(&value);
        } else if code == b"2" && section.is_none() {
            section = Some(value.to_vec());
        } else if code == b"9" && value == b"$HANDSEED" && i + 3 < out.len() {
            seed = if trim(&out[i + 2]) == b"5" {
                Some(i + 3)
            } else {
                None
            };
        } else if code == b"5"
            && expecting
            && section.as_deref().is_some_and(|s| {
                [b"TABLES".as_slice(), b"BLOCKS", b"ENTITIES", b"OBJECTS"].contains(&s)
            })
        {
            spots.push((i + 1, value.to_vec()));
            expecting = false;
        }
        i += 2;
    }
    let mut seen = HashSet::new();
    let mut collisions = Vec::new();
    let mut max = 0u64;
    for (index, h) in &spots {
        if !seen.insert(h.clone()) {
            collisions.push(*index)
        }
        if let Ok(s) = std::str::from_utf8(h) {
            if let Ok(n) =
                u64::from_str_radix(s.trim_start_matches('+').trim_start_matches("0x"), 16)
            {
                if n < (1u64 << 32) {
                    max = max.max(n)
                }
            }
        }
    }
    let count = collisions.len();
    let mut next = max + 1;
    for index in collisions {
        while seen.contains(format!("{next:X}").as_bytes()) {
            next += 1
        }
        let fresh = format!("{next:X}").into_bytes();
        seen.insert(fresh.clone());
        out[index] = fresh;
        next += 1;
    }
    if count > 0 {
        if let Some(index) = seed {
            out[index] = format!("{next:X}").into_bytes()
        }
    }
    let capacity = out
        .iter()
        .try_fold(out.len().saturating_sub(1), |n, l| n.checked_add(l.len()))
        .ok_or("DXF repaired size overflow")?;
    if capacity > MAX_DXF_BYTES {
        return Err("DXF repaired byte limit exceeded".into());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| "DXF repaired allocation refused")?;
    for (i, l) in out.iter().enumerate() {
        if i > 0 {
            result.push(b'\n')
        }
        result.extend_from_slice(l)
    }
    Ok((result, nulls, count))
}
