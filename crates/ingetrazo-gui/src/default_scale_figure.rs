//! Source new-document scale figure, owned by the document and outside history.
use glam::DVec3;
use ingetrazo_core::{groups::Group, i18n::tr, mesh::Mesh};
use serde_json::json;
use std::io::Cursor;
const ENGINEER: &[u8] = include_bytes!("../resources/components/ingeniero.png");
const PERSON: &[u8] = include_bytes!("../resources/components/person_billboard.png");

pub(crate) struct StartupFigure {
    pub document: uuid::Uuid,
    pub group: uuid::Uuid,
    pub scene_version: u64,
    pub model_revision: u64,
    pub texture: String,
    pub label: &'static str,
}
fn make(bytes: &[u8], key: &str, height: f64, name: &str) -> Result<Group, String> {
    // Bundled immutable, authenticated PNGs: inspect header without decoding an
    // extra full RGBA copy. Actual renderer uses the existing central decoder.
    let (width, pixels_high) =
        image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|e| e.to_string())?;
    if width == 0 || pixels_high == 0 {
        return Err("Scale figure dimensions refused".into());
    }
    let width = height * (f64::from(width) / f64::from(pixels_high));
    let half = (width / 2.) as f32;
    let x = -0.65f32;
    let y = -0.60f32;
    let z = height as f32;
    // Source adds two QVector3Ds in float32 after each constructor narrows.
    let mut mesh = Mesh::new();
    let points = [
        [x - half, y, 0.],
        [x + half, y, 0.],
        [x + half, y, z],
        [x - half, y, z],
    ];
    let vertices = points
        .into_iter()
        .map(|p| mesh.add_vertex(DVec3::from_array(p.map(f64::from))))
        .collect();
    let face = mesh
        .add_face(vertices, Vec::new())
        .ok_or("Scale figure face refused")?;
    mesh.faces[face]
        .attrs
        .insert("texture".into(), json!({"path":key,"sw":width,"sh":height}));
    let mut group = Group::with_mesh(name, mesh);
    group.billboard = true;
    Ok(group)
}
impl crate::IngeTrazoApp {
    pub(crate) fn insert_default_scale_figure(&mut self) -> Result<(), String> {
        self.startup_scale_figure = None;
        if !self.document_session.preferences.scale_figure_enabled() {
            return Ok(());
        }
        let engineer_key = "textures/ingeniero.png";
        let person_key = "textures/person_billboard.png";
        let (group, key, bytes, label) = match make(ENGINEER, engineer_key, 1.70, &tr("Engineer")) {
            Ok(group) => (group, engineer_key, ENGINEER, "Engineer"),
            Err(_) => (
                make(PERSON, person_key, 1.75, &tr("Person"))?,
                person_key,
                PERSON,
                "Person",
            ),
        };
        self.scene
            .groups
            .try_reserve(1)
            .map_err(|_| "Scale figure group admission refused")?;
        self.scene
            .embedded_textures
            .try_reserve(1)
            .map_err(|_| "Scale figure resource admission refused")?;
        let mut resource = Vec::new();
        resource
            .try_reserve_exact(bytes.len())
            .map_err(|_| "Scale figure bytes admission refused")?;
        resource.extend_from_slice(bytes);
        let group_id = group.id;
        self.scene.embedded_textures.insert(key.into(), resource);
        self.scene.groups.push(group);
        self.scene.bump_version();
        self.startup_scale_figure = Some(StartupFigure {
            document: self.clipboard_document,
            group: group_id,
            scene_version: self.scene.version,
            model_revision: self.model_revision,
            texture: key.into(),
            label,
        });
        Ok(())
    }
    pub(crate) fn reconcile_startup_scale_figure(&mut self) {
        let Some(owner) = self.startup_scale_figure.as_ref() else {
            return;
        };
        // Preferences are loaded after App construction. Only the pristine,
        // owned startup document is eligible; loaded/edited records are untouched.
        if owner.document != self.clipboard_document
            || owner.scene_version != self.scene.version
            || owner.model_revision != self.model_revision
            || self.current_file.is_some()
            || !self.undo_stack.is_empty()
            || !self.redo_stack.is_empty()
            || self.scene.groups.len() != 1
            || self.scene.groups[0].id != owner.group
            || !self.scene.mesh.vertices.is_empty()
            || self.document_dirty()
        {
            self.startup_scale_figure = None;
            return;
        }
        if self.document_session.preferences.scale_figure_enabled() {
            self.scene.groups[0].name = tr(owner.label);
        } else {
            let owner = self.startup_scale_figure.take().unwrap();
            self.scene.groups.clear();
            self.scene.embedded_textures.remove(&owner.texture);
            self.scene.bump_version();
        }
    }
}
