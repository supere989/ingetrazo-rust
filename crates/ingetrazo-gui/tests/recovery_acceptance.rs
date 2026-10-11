use glam::DVec3;
use ingetrazo_gui::{IngeTrazoApp,session_state::SessionStore,document_session::DocumentAction};
use std::{path::PathBuf,time::{Instant,Duration}};
#[test] fn crash_helper(){
 let Ok(root)=std::env::var("INGETRAZO_ACCEPTANCE_CRASH_ROOT") else{return};let root=PathBuf::from(root);
 let mut app=IngeTrazoApp::new();app.attach_session_store(SessionStore::isolated(&root));app.open_document(&root.join("original.igz"));
 app.scene.mesh.vertices[0].position=DVec3::new(9.,8.,7.);app.document_session.preferences.autosave_minutes=11;
 app.document_session.store.as_ref().unwrap().save_preferences(&app.document_session.preferences).unwrap();
 assert!(app.autosave_if_due(Instant::now()+Duration::from_secs(1200),false).unwrap());std::process::exit(37);
}
#[test] fn crash_restart_retains_original_and_failed_close(){
 let root=std::env::temp_dir().join(format!("ingetrazo-independent-recovery-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();
 let source=root.join("original.igz");let mut scene=ingetrazo_core::scene::Scene::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
 ingetrazo_formats::igz::save_igz(&scene,&source).unwrap();let original=std::fs::read(&source).unwrap();
 let status=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","crash_helper","--nocapture"]).env("INGETRAZO_ACCEPTANCE_CRASH_ROOT",&root).status().unwrap();assert_eq!(status.code(),Some(37));
 let mut app=IngeTrazoApp::new();app.attach_session_store(SessionStore::isolated(&root));assert_eq!(app.document_session.preferences.autosave_minutes,11);
 let records=app.document_session.store.as_ref().unwrap().recovery_candidates(false).unwrap();assert_eq!(records.len(),1);assert_eq!(records[0].document_path.as_ref(),Some(&source));
 app.request_document_action(DocumentAction::Recover(records[0].clone()));assert!(app.current_file.is_none());assert!(app.document_dirty());assert_eq!(app.scene.mesh.vertices[0].position,DVec3::new(9.,8.,7.));
 let ctx=egui::Context::default();let mut input=egui::RawInput::default();input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().events.push(egui::ViewportEvent::Close);
 let out=ctx.run(input,|ctx|app.document_session_ui(ctx));assert!(out.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c|matches!(c,egui::ViewportCommand::CancelClose)));
 app.save_to_path(&root.join("missing/failure.igz"));assert!(app.document_dirty());assert!(!app.document_session.close_allowed);assert!(!app.document_session.confirm_unsaved);assert!(app.document_session.pending.is_none());assert_eq!(std::fs::read(&source).unwrap(),original);
 // Original failed Save returns from the prompt; it must not leave Close armed.
 let out=ctx.run(egui::RawInput::default(),|ctx|app.document_session_ui(ctx));assert!(!out.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c|matches!(c,egui::ViewportCommand::Close)));
 app.cancel_document_action();assert_eq!(app.scene.mesh.vertices[0].position,DVec3::new(9.,8.,7.));app.request_document_action(DocumentAction::Close);app.discard_document_action();
 let out=ctx.run(egui::RawInput::default(),|ctx|app.document_session_ui(ctx));assert!(out.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|c|matches!(c,egui::ViewportCommand::Close)));std::fs::remove_dir_all(root).unwrap();
}
