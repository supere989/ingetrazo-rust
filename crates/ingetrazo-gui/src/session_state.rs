//! Durable preferences and recovery copies, separate from project documents.
use std::{collections::BTreeMap,path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
use serde::{Serialize,Deserialize};
use ingetrazo_core::{scene::Scene,units::LengthUnit};
use ingetrazo_formats::igz::{save_igz,load_igz};

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct Preferences {
    pub language: Option<String>,
    pub walk_eye_height: f64,
    pub look_sensitivity: u8,
    pub new_document_unit: LengthUnit,
    pub autosave: bool,
    pub autosave_minutes: u32,
    pub undo_steps: usize,
    pub visual_style: String,
    pub profile_width: f32,
    pub line_width: f32,
    pub shadows: bool,
    pub show_toolbar: bool,
    pub show_tray: bool,
    pub recent_files: Vec<PathBuf>,
    #[serde(flatten)]
    pub extra: BTreeMap<String,serde_json::Value>,
}
impl Default for Preferences {
    fn default()->Self {Self{walk_eye_height:1.68,look_sensitivity:25,language:None,new_document_unit:LengthUnit::Meters,autosave:true,autosave_minutes:5,undo_steps:200,
        visual_style:"shaded".into(),profile_width:2.,line_width:1.,shadows:false,show_toolbar:true,show_tray:true,
        recent_files:vec![],extra:BTreeMap::new()}}
}
impl Preferences {
    pub fn clamp(&mut self) {
        if !self.walk_eye_height.is_finite() || !(0.05..=100.).contains(&self.walk_eye_height) { self.walk_eye_height=1.68; }
        if !(1..=100).contains(&self.look_sensitivity) { self.look_sensitivity=25; }
        self.autosave_minutes=self.autosave_minutes.clamp(1,60);self.undo_steps=self.undo_steps.clamp(1,1000);
        self.profile_width=if self.profile_width.is_finite(){self.profile_width.clamp(1.,6.)}else{2.};
        self.line_width=if self.line_width.is_finite(){self.line_width.clamp(0.5,4.)}else{1.};
        self.recent_files.truncate(10);
    }
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
pub struct RecoveryRecord {
    pub slot: PathBuf,
    pub document_path: Option<PathBuf>,
    pub owner_pid: u32,
    pub session_id: uuid::Uuid,
    pub updated_ms: u64,
}
#[derive(Debug,Clone)]
pub struct SessionStore {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub session_id: uuid::Uuid,
    pub settings_writable: bool,
}
fn timestamp()->u64 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().min(u64::MAX as u128) as u64}
fn atomic_json(path:&Path,value:&impl Serialize)->Result<(),String> {
    let parent=path.parent().ok_or("settings path has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;
    let bytes=serde_json::to_vec_pretty(value).map_err(|e|e.to_string())?;
    let temp=parent.join(format!(".state-{}.tmp",uuid::Uuid::new_v4()));
    {
        use std::io::Write;
        let mut file=std::fs::File::create(&temp).map_err(|e|e.to_string())?;
        if let Err(e)=file.write_all(&bytes).and_then(|_|file.sync_all()) {let _=std::fs::remove_file(&temp);return Err(e.to_string());}
    }
    if let Err(e)=std::fs::rename(&temp,path) {let _=std::fs::remove_file(temp);return Err(e.to_string());}
    #[cfg(unix)]
    std::fs::File::open(parent).and_then(|f|f.sync_all()).map_err(|e|e.to_string())?;
    Ok(())
}
fn absolute(path:&Path)->Result<PathBuf,String> {
    if path.is_absolute(){Ok(path.to_path_buf())}else{std::env::current_dir().map(|d|d.join(path)).map_err(|e|e.to_string())}
}
fn owner_alive(pid:u32)->bool {
    #[cfg(target_os="linux")]
    {Path::new("/proc").join(pid.to_string()).exists()}
    #[cfg(not(target_os="linux"))]
    {pid==std::process::id()}
}
impl SessionStore {
    pub fn isolated(root:&Path)->Self {Self{config_dir:root.join("config"),data_dir:root.join("data"),session_id:uuid::Uuid::new_v4(),settings_writable:true}}
    pub fn user_default()->Self {
        let home=std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(||PathBuf::from("."));
        #[cfg(target_os="windows")]
        let (config,data)=(std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(||home.join("AppData/Roaming")),
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(||home.join("AppData/Local")));
        #[cfg(target_os="macos")]
        let (config,data)=(home.join("Library/Application Support"),home.join("Library/Application Support"));
        #[cfg(not(any(target_os="windows",target_os="macos")))]
        let (config,data)=(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(||home.join(".config")),
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(||home.join(".local/share")));
        Self{config_dir:config.join("ingetrazo-rust"),data_dir:data.join("ingetrazo-rust"),session_id:uuid::Uuid::new_v4(),settings_writable:true}
    }
    pub fn load_preferences(&mut self)->Result<Preferences,String> {
        let path=self.config_dir.join("settings.json");
        let bytes=match std::fs::read(&path) {Ok(b)=>b,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Preferences::default()),Err(e)=>{self.settings_writable=false;return Err(e.to_string());}};
        match serde_json::from_slice::<Preferences>(&bytes) {
            Ok(mut prefs)=>{prefs.clamp();Ok(prefs)},
            Err(e)=>{self.settings_writable=false;Err(format!("preferences preserved at {}: {e}",path.display()))}
        }
    }
    pub fn save_preferences(&self,prefs:&Preferences)->Result<(),String> {
        if !self.settings_writable{return Err("unreadable preferences are preserved; refusing to overwrite".into());}
        let mut prefs=prefs.clone();prefs.clamp();atomic_json(&self.config_dir.join("settings.json"),&prefs)
    }
    pub fn slot_for(&self,path:Option<&Path>)->Result<PathBuf,String> {
        let name=if let Some(path)=path {
            let absolute=absolute(path)?;
            // Stable identity independent of process-randomized hashers. Session
            // UUID also prevents different windows sharing a writable slot.
            let mut hash=0xcbf29ce484222325u64;
            for byte in absolute.as_os_str().as_encoded_bytes() {hash^=*byte as u64;hash=hash.wrapping_mul(0x100000001b3);}
            format!("document-{hash:016x}-{}.igz",self.session_id)
        }else{format!("untitled-{}.igz",self.session_id)};
        Ok(self.data_dir.join("autosave").join(name))
    }
    pub fn autosave(&self,scene:&Scene,path:Option<&Path>)->Result<RecoveryRecord,String> {
        let slot=self.slot_for(path)?;std::fs::create_dir_all(slot.parent().unwrap()).map_err(|e|e.to_string())?;
        let record=RecoveryRecord{slot:slot.clone(),document_path:path.map(absolute).transpose()?,owner_pid:std::process::id(),session_id:self.session_id,updated_ms:timestamp()};
        // Payload first. If metadata fails, keep the recovery payload rather than
        // damaging the original document; the error is visible to the caller.
        save_igz(scene,&slot).map_err(|e|e.to_string())?;
        atomic_json(&slot.with_extension("json"),&record)?;Ok(record)
    }
    pub fn recovery_candidates(&self,discarded:bool)->Result<Vec<RecoveryRecord>,String> {
        let folder=self.data_dir.join("autosave").join(if discarded{"discarded"}else{""});
        let entries=match std::fs::read_dir(folder) {Ok(e)=>e,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>return Ok(vec![]),Err(e)=>return Err(e.to_string())};
        let mut records=vec![];
        for entry in entries {
            let path=entry.map_err(|e|e.to_string())?.path();
            if path.extension().and_then(|e|e.to_str())!=Some("igz"){continue;}
            let record=std::fs::read(path.with_extension("json")).ok()
                .and_then(|bytes|serde_json::from_slice::<RecoveryRecord>(&bytes).ok());
            let mut record=record.unwrap_or_else(||RecoveryRecord {
                slot:path.clone(),document_path:None,owner_pid:0,session_id:uuid::Uuid::nil(),
                updated_ms:std::fs::metadata(&path).ok().and_then(|m|m.modified().ok())
                    .and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_millis().min(u64::MAX as u128) as u64).unwrap_or(0)
            });
            // Metadata failure must not hide a successfully written payload.
            record.slot=path;
            if discarded || !owner_alive(record.owner_pid) {records.push(record);}

        }
        records.sort_by_key(|r|std::cmp::Reverse(r.updated_ms));Ok(records)
    }
    pub fn recover(&self,record:&RecoveryRecord)->Result<Scene,String> {load_igz(&record.slot).map_err(|e|e.to_string())}
    pub fn retire(&self,slot:&Path)->Result<(),String> {
        if !slot.is_file(){return Ok(());}
        let folder=self.data_dir.join("autosave/discarded");std::fs::create_dir_all(&folder).map_err(|e|e.to_string())?;
        let target=folder.join(format!("retired-{}-{}.igz",timestamp(),uuid::Uuid::new_v4()));
        std::fs::rename(slot,&target).map_err(|e|e.to_string())?;
        let metadata=slot.with_extension("json");
        if metadata.is_file(){std::fs::rename(metadata,target.with_extension("json")).map_err(|e|e.to_string())?;}
        let records=self.recovery_candidates(true)?;
        for record in records.into_iter().skip(20) {
            std::fs::remove_file(&record.slot).map_err(|e|e.to_string())?;
            let _=std::fs::remove_file(record.slot.with_extension("json"));
        }
        Ok(())
    }
    pub fn retire_document(&self,path:Option<&Path>)->Result<(),String> {self.retire(&self.slot_for(path)?)}
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    fn root()->PathBuf {std::env::temp_dir().join(format!("ingetrazo-session-test-{}",uuid::Uuid::new_v4()))}
    #[test]
    fn preferences_keep_unknown_settings_and_corrupt_source_is_never_overwritten() {
        let root=root();let mut store=SessionStore::isolated(&root);
        let mut prefs=Preferences::default();prefs.language=Some("es".into());prefs.extra.insert("future_setting".into(),serde_json::json!({"opaque":[1,2]}));
        store.save_preferences(&prefs).unwrap();let loaded=store.load_preferences().unwrap();assert_eq!(loaded,prefs);
        let path=store.config_dir.join("settings.json");std::fs::write(&path,b"corrupt original").unwrap();
        assert!(store.load_preferences().is_err());assert!(store.save_preferences(&prefs).is_err());
        assert_eq!(std::fs::read(&path).unwrap(),b"corrupt original");std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn slots_do_not_collide_across_paths_or_live_windows_and_retirement_keeps_recoverable_copy() {
        let root=root();let store=SessionStore::isolated(&root);let other=SessionStore::isolated(&root);
        let a=root.join("a/casa.igz");let b=root.join("b/casa.igz");assert_ne!(store.slot_for(Some(&a)).unwrap(),store.slot_for(Some(&b)).unwrap());
        assert_ne!(store.slot_for(Some(&a)).unwrap(),other.slot_for(Some(&a)).unwrap());
        let scene=Scene::new();let record=store.autosave(&scene,Some(&a)).unwrap();
        assert!(store.recovery_candidates(false).unwrap().is_empty()); // active process is not a crash
        store.retire(&record.slot).unwrap();assert!(!record.slot.exists());
        let retired=store.recovery_candidates(true).unwrap();assert_eq!(retired.len(),1);assert!(store.recover(&retired[0]).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn crash_writer_helper() {
        let Some(root)=std::env::var_os("INGETRAZO_CRASH_FIXTURE_ROOT") else{return;};
        let store=SessionStore::isolated(Path::new(&root));let mut scene=Scene::new();
        let verts=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(verts.to_vec(),vec![]).unwrap();
        scene.mesh.faces[0].attrs.insert("crash_edit".into(),serde_json::json!(17));
        store.autosave(&scene,Some(&Path::new(&root).join("original.igz"))).unwrap();
        // Terminate without any application clean-close retirement.
        std::process::exit(0);
    }
    #[test]
    fn separate_process_crash_restart_recovers_latest_geometry_without_touching_original() {
        let root=root();std::fs::create_dir_all(&root).unwrap();let original=root.join("original.igz");std::fs::write(&original,b"original must remain unchanged").unwrap();
        let status=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","session_state::tests::crash_writer_helper","--nocapture"])
            .env("INGETRAZO_CRASH_FIXTURE_ROOT",&root).status().unwrap();assert!(status.success());
        let store=SessionStore::isolated(&root);let records=store.recovery_candidates(false).unwrap();assert_eq!(records.len(),1);
        let recovered=store.recover(&records[0]).unwrap();assert_eq!(recovered.mesh.faces.len(),1);assert_eq!(recovered.mesh.faces[0].attrs["crash_edit"],17);
        assert_eq!(std::fs::read(original).unwrap(),b"original must remain unchanged");
        std::fs::remove_dir_all(root).unwrap();
    }
}
