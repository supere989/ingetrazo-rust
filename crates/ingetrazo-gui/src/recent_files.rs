//! Native document history; imports and missing files are not menu entries.
use crate::IngeTrazoApp;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct MenuCache {
    pub frame: Option<u64>,
    pub indices: Vec<usize>,
}

fn resolved_path(path: &Path) -> PathBuf {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    });
    // Match normal document paths rather than exposing Windows API prefixes.
    #[cfg(windows)]
    {
        use std::{
            ffi::OsString,
            path::{Component, Prefix},
        };
        let mut parts = path.components();
        if let Some(Component::Prefix(prefix)) = parts.next() {
            let mut normal = match prefix.kind() {
                Prefix::VerbatimDisk(disk) => OsString::from(format!("{}:", disk as char)),
                Prefix::VerbatimUNC(server, share) => {
                    let mut value = OsString::from(r"\\");
                    value.push(server);
                    value.push(r"\");
                    value.push(share);
                    value
                }
                _ => return path,
            };
            normal.push(parts.as_path());
            return PathBuf::from(normal);
        }
    }
    path
}

impl IngeTrazoApp {
    pub(crate) fn remember_recent_document(&mut self, path: &Path) {
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("igz"))
        {
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_state::SessionStore;
    fn text(out: &egui::FullOutput, needle: &str) -> egui::Pos2 {
        out.shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == needle)
                        .then_some(t.pos + t.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing actual UI text {needle}; got {:?}",
                    out.shapes
                        .iter()
                        .filter_map(|s| if let egui::Shape::Text(t) = &s.shape {
                            Some(t.galley.job.text.clone())
                        } else {
                            None
                        })
                        .collect::<Vec<_>>()
                )
            })
    }
    fn frame(
        a: &mut IngeTrazoApp,
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900., 600.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::TopBottomPanel::top("actual-menu")
                    .show(ctx, |ui| crate::menu::render_menu_bar(a, ui));
                a.document_session_ui(ctx);
            },
        )
    }
    fn click(
        a: &mut IngeTrazoApp,
        ctx: &egui::Context,
        t: &mut f64,
        p: egui::Pos2,
    ) -> egui::FullOutput {
        let mut result = None;
        for pressed in [true, false] {
            *t += 0.1;
            result = Some(frame(
                a,
                ctx,
                *t,
                vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            ));
        }
        result.unwrap()
    }
    fn open(a: &mut IngeTrazoApp, ctx: &egui::Context, t: &mut f64) -> egui::FullOutput {
        *t += 0.1;
        let out = frame(a, ctx, *t, vec![]);
        let file = text(&out, "File");
        *t += 0.1;
        frame(a, ctx, *t, vec![egui::Event::PointerMoved(file)]);
        click(a, ctx, t, file);
        *t += 0.1;
        let out = frame(a, ctx, *t, vec![]);
        let recent = text(&out, "Open Recent");
        *t += 0.1;
        frame(a, ctx, *t, vec![egui::Event::PointerMoved(recent)]);
        *t += 0.4;
        frame(a, ctx, *t, vec![egui::Event::PointerMoved(recent)])
    }
    #[test]
    fn actual_popup_retirement_missing_warning_clear_and_restart() {
        let root = std::env::temp_dir().join(format!("recent-ui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("document.igz");
        let mut a = IngeTrazoApp::new();
        a.set_language("en");
        a.attach_session_store(SessionStore::isolated(&root));
        a.save_to_path(&path);
        let ctx = egui::Context::default();
        let mut t = 0.;
        let out = open(&mut a, &ctx, &mut t);
        assert_eq!(a.document_session.recent_menu.indices, vec![0]);
        assert!(
            a.document_session.recent_menu.frame.is_some(),
            "open popup cache retired prematurely"
        );
        let label = format!("1  document.igz  —  {}", root.display());
        let entry = text(&out, &label);
        std::fs::remove_file(&path).unwrap();
        // Warm popup retains entries without per-frame filesystem polling.
        t += 0.1;
        frame(&mut a, &ctx, t, vec![egui::Event::PointerMoved(entry)]);
        assert_eq!(a.document_session.recent_menu.indices, vec![0]);
        let before = a.scene.clone();
        click(&mut a, &ctx, &mut t, entry);
        assert_eq!(a.scene, before);
        assert!(a.document_session.recent_missing.is_some());
        t += 0.1;
        frame(
            &mut a,
            &ctx,
            t,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(a.document_session.recent_missing.is_none());
        let out = open(&mut a, &ctx, &mut t);
        text(&out, "(no recent documents)");
        assert!(a.document_session.recent_menu.indices.is_empty());
        // Outside dismissal retires ownership immediately; reopening refreshes.
        click(&mut a, &ctx, &mut t, egui::pos2(850., 550.));
        assert!(a.document_session.recent_menu.frame.is_none());
        a.save_to_path(&path);
        let out = open(&mut a, &ctx, &mut t);
        text(&out, &label);
        t += 0.1;
        frame(
            &mut a,
            &ctx,
            t,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(a.document_session.recent_menu.frame.is_none());
        std::fs::remove_file(&path).unwrap();
        let out = open(&mut a, &ctx, &mut t);
        text(&out, "(no recent documents)");
        click(&mut a, &ctx, &mut t, egui::pos2(850., 550.));
        assert!(a.document_session.recent_menu.frame.is_none());
        a.save_to_path(&path);
        let out = open(&mut a, &ctx, &mut t);
        let clear = text(&out, "Clear list");
        let before = a.scene.clone();
        let history = (a.undo_stack.len(), a.redo_stack.len());
        click(&mut a, &ctx, &mut t, clear);
        assert!(a.recent_files.is_empty());
        assert_eq!(a.scene, before);
        assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
        // Actual application keyboard admission while the blocking warning
        // owns input, including its dismissal frame.
        let v0 = a.scene.mesh.add_vertex(glam::DVec3::X);
        a.save_undo_state();
        let v1 = a.scene.mesh.add_vertex(glam::DVec3::Y);
        a.scene.mesh.add_edge(v0, v1);
        a.scene
            .select(ingetrazo_core::scene::SelectionItem::Edge(0));
        a.set_tool(ingetrazo_tools::base::ToolType::Line);
        a.dispatch_tool_click(&ingetrazo_tools::base::ToolContext::new(
            glam::DVec3::Z,
            ingetrazo_core::math::Ray::new(glam::DVec3::Z, -glam::DVec3::Z),
        ));
        let pending_anchor = a.active_tool.snap_anchor();
        assert!(pending_anchor.is_some());
        a.open_recent_document(&root.join("absent.igz"));
        let before = a.scene.clone();
        let history = (a.undo_stack.len(), a.redo_stack.len());
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        for event in [
            key(egui::Key::Z, egui::Modifiers::CTRL),
            key(egui::Key::Delete, egui::Modifiers::NONE),
            key(egui::Key::Escape, egui::Modifiers::NONE),
        ] {
            t += 0.1;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900., 600.),
                    )),
                    time: Some(t),
                    events: vec![event],
                    ..Default::default()
                },
                |ctx| a.update_ui(ctx),
            );
            assert_eq!(a.scene.mesh, before.mesh);
            assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
            assert_eq!(a.active_tool.snap_anchor(), pending_anchor);
        }
        assert!(a.document_session.recent_missing.is_none());
        let mut restarted = IngeTrazoApp::new();
        restarted.attach_session_store(SessionStore::isolated(&root));
        assert!(restarted.recent_files.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
