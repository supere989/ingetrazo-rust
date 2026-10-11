//! Original IGZ recursive texture packing, on the serialized payload only.
//! Existing archive bytes stay borrowed; each new file is read once per save.
use ingetrazo_core::scene::Scene;
use serde_json::Value;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    path::Path,
};

pub(super) fn file_name(name: &str) -> String {
    let mut safe: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || " ._-".contains(*c))
        .collect();
    safe = safe.trim_matches([' ', '.']).to_owned();
    loop {
        let bytes = safe.as_bytes();
        if bytes.len() >= 17
            && bytes[16] == b'-'
            && bytes[..16]
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            safe.drain(..17);
        } else {
            break;
        }
    }
    safe = safe.trim_matches([' ', '.']).to_owned();
    if safe.chars().count() > 64 {
        if let Some((stem, ext)) = safe.rsplit_once('.') {
            let n = ext.chars().count();
            if n > 0 && n <= 5 && ext.chars().all(char::is_alphanumeric) {
                safe = format!(
                    "{}.{}",
                    stem.chars()
                        .take(64 - n - 1)
                        .collect::<String>()
                        .trim_end_matches([' ', '.']),
                    ext
                );
            } else {
                safe = safe
                    .chars()
                    .take(64)
                    .collect::<String>()
                    .trim_end_matches([' ', '.'])
                    .to_owned();
            }
        } else {
            safe = safe
                .chars()
                .take(64)
                .collect::<String>()
                .trim_end_matches([' ', '.'])
                .to_owned();
        }
    }
    if safe.is_empty() {
        "texture.png".into()
    } else {
        safe
    }
}

pub(super) fn pack<'a>(
    scene: &'a Scene,
    document: &mut Value,
) -> Result<BTreeMap<String, Cow<'a, [u8]>>, super::IgzError> {
    let mut blobs: BTreeMap<_, _> = scene
        .embedded_textures
        .iter()
        .map(|(k, v)| (k.clone(), Cow::Borrowed(v.as_slice())))
        .collect();
    let mut resolved: HashMap<String, Option<String>> = HashMap::new();
    fn member<'a>(
        src: &str,
        scene: &'a Scene,
        blobs: &mut BTreeMap<String, Cow<'a, [u8]>>,
        resolved: &mut HashMap<String, Option<String>>,
    ) -> Result<Option<String>, super::IgzError> {
        if let Some(result) = resolved.get(src) {
            return Ok(result.clone());
        }
        let result = if scene.embedded_textures.contains_key(src) {
            Some(src.to_owned())
        } else if let Ok(data) = std::fs::read(src) {
            let digest = sha1_smol::Sha1::from(data.as_slice()).digest().to_string();
            let name = Path::new(src)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let member = format!("textures/{}-{}", &digest[..16], file_name(&name));
            // Identical bytes/name reuse one immutable member. A cryptographic
            // collision is not permitted to overwrite another live image.
            if blobs
                .get(&member)
                .is_some_and(|old| old.as_ref() != data.as_slice())
            {
                return Err(super::IgzError::InvalidDocument(format!(
                    "Conflicting embedded texture member {member}"
                )));
            } else {
                blobs.entry(member.clone()).or_insert(Cow::Owned(data));
                Some(member)
            }
        } else {
            None
        }; // Source retains unreadable file references.
        resolved.insert(src.into(), result.clone());
        Ok(result)
    }
    fn walk<'a>(
        node: &mut Value,
        scene: &'a Scene,
        blobs: &mut BTreeMap<String, Cow<'a, [u8]>>,
        resolved: &mut HashMap<String, Option<String>>,
    ) -> Result<(), super::IgzError> {
        match node {
            Value::Object(object) => {
                for (key, value) in object.iter_mut() {
                    if key == "texture" {
                        if let Some(texture) = value.as_object_mut() {
                            for (path, embed) in [("path", "embed"), ("base", "base_embed")] {
                                if let Some(src) = texture
                                    .get(path)
                                    .and_then(Value::as_str)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_owned)
                                {
                                    if let Some(name) = member(&src, scene, blobs, resolved)? {
                                        texture.remove(path);
                                        texture.insert(embed.into(), Value::String(name));
                                    }
                                }
                            }
                        }
                    } else if !["vertices", "holes", "a", "b", "edges"].contains(&key.as_str()) {
                        walk(value, scene, blobs, resolved)?;
                    }
                }
            }
            Value::Array(array) => {
                if array
                    .first()
                    .is_some_and(|v| !v.is_array() && !v.is_object())
                {
                    return Ok(());
                }
                for value in array {
                    walk(value, scene, blobs, resolved)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(document, scene, &mut blobs, &mut resolved)?;
    Ok(blobs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recursive_assets_are_packed_without_mutating_live_payload() {
        let dir = std::env::temp_dir().join(format!("igz-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("0123456789abcdef-paint.png");
        std::fs::write(&file, b"literal bytes").unwrap();
        let scene = Scene::new();
        let mut doc = serde_json::json!({"faces":[{"texture":{"path":file,"base":file},"back":{"texture":{"path":file}}}],"image_planes":[{"texture":{"path":file}}]});
        let original = doc.clone();
        let blobs = pack(&scene, &mut doc).unwrap();
        assert_eq!(blobs.len(), 1);
        let name = blobs.keys().next().unwrap();
        assert!(name.ends_with("-paint.png"));
        assert_eq!(doc["faces"][0]["texture"]["embed"], name.as_str());
        assert_eq!(doc["faces"][0]["texture"]["base_embed"], name.as_str());
        assert_eq!(doc["faces"][0]["back"]["texture"]["embed"], name.as_str());
        assert_eq!(doc["image_planes"][0]["texture"]["embed"], name.as_str());
        assert!(original["faces"][0]["texture"]["path"].is_string());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unreadable_file_reference_is_retained_as_in_the_original() {
        let scene = Scene::new();
        let mut doc = serde_json::json!({"texture":{"path":"/missing/igz-file.png","base":"/missing/igz-base.png"}});
        let before = doc.clone();
        assert!(pack(&scene, &mut doc).unwrap().is_empty());
        assert_eq!(before, doc);
    }
    #[test]
    fn conflicting_member_refuses_before_destination_or_live_document_changes() {
        use glam::DVec3;
        let dir = std::env::temp_dir().join(format!("igz-refusal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("paint.png");
        std::fs::write(&file, b"new bytes").unwrap();
        let digest = sha1_smol::Sha1::from(b"new bytes".as_slice())
            .digest()
            .to_string();
        let mut scene = Scene::new();
        scene.embedded_textures.insert(
            format!("textures/{}-paint.png", &digest[..16]),
            b"old conflicting bytes".to_vec(),
        );
        let ring = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| scene.mesh.add_vertex(p));
        let face = scene.mesh.add_face(ring.to_vec(), vec![]).unwrap();
        scene.mesh.faces[face]
            .attrs
            .insert("texture".into(), serde_json::json!({"path":file}));
        let before = scene.clone();
        let dest = dir.join("saved.igz");
        std::fs::write(&dest, b"existing document").unwrap();
        let error = super::super::save_igz(&scene, &dest).unwrap_err();
        assert!(error.to_string().contains("Conflicting embedded texture"));
        assert_eq!(std::fs::read(&dest).unwrap(), b"existing document");
        assert_eq!(scene, before);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
