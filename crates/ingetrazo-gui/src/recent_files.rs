//! Native document history; imports and missing files are not menu entries.
use std::path::{Path, PathBuf};
use crate::IngeTrazoApp;

#[derive(Default)]
pub(crate) struct MenuCache {
    pub frame: Option<u64>,
    pub indices: Vec<usize>,
}

fn resolved_path(path: &Path) -> PathBuf {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() { path.to_path_buf() } else {
            std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
        }
    });
    // Match normal document paths rather than exposing Windows API prefixes.
    #[cfg(windows)] {
        use std::{ffi::OsString, path::{Component, Prefix}};
        let mut parts = path.components();
        if let Some(Component::Prefix(prefix)) = parts.next() {
            let mut normal = match prefix.kind() {
                Prefix::VerbatimDisk(disk) => OsString::from(format!("{}:", disk as char)),
                Prefix::VerbatimUNC(server, share) => {
                    let mut value = OsString::from(r"\\"); value.push(server); value.push(r"\"); value.push(share); value
                },
                _ => return path,
            };
            normal.push(parts.as_path()); return PathBuf::from(normal);
        }
    }
    path
}

impl IngeTrazoApp {
    pub(crate) fn remember_recent_document(&mut self, path: &Path) {
        if !path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("igz")) { return; }
        let path = resolved_path(path);
        self.recent_files.retain(|old| old != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(10);
        self.document_session.recent_menu.frame = None;
    }

    pub fn clear_recent_files(&mut self) {
        self.recent_files.clear();
        self.document_session.recent_menu.frame = None;
        self.persist_preferences();
    }

    pub fn open_recent_document(&mut self, path: &Path) {
        // A deleted entry must not finish a live component edit or ask to
        // discard it. The source checks existence before either transition.
        if !path.exists() {
            self.document_session.recent_missing = Some(path.to_path_buf());
            return;
        }
        self.document_session.recent_missing = None;
        self.open_document(path);
    }
}
