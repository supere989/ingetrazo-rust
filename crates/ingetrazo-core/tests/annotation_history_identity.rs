use ingetrazo_core::composition::Sheet;
use serde_json::json;

#[test]
fn identities_are_runtime_only_and_do_not_change_document_equality() {
    let mut sheet = Sheet::default();
    sheet.native_document = json!({"cotas":[{"x_mm":40.0},{"x_mm":80.0}]});
    let before = sheet.clone();
    let encoded = serde_json::to_value(&sheet).unwrap();
    sheet.ensure_annotation_ids().unwrap();
    assert_eq!(sheet, before);
    assert_eq!(serde_json::to_value(&sheet).unwrap(), encoded);
    let id = sheet.annotation_id(0, 0).unwrap();
    sheet.ensure_annotation_ids().unwrap();
    assert_eq!(sheet.annotation_id(0, 0), Some(id));
    assert_eq!(sheet.clone().annotation_id(0, 0), Some(id));
    let mut reopened: Sheet = serde_json::from_value(encoded).unwrap();
    reopened.ensure_annotation_ids().unwrap();
    assert_ne!(reopened.annotation_id(0, 0), Some(id));
}

#[test]
fn undoing_earlier_field_edit_preserves_delete_undo_append_order() {
    let mut initial = Sheet::default();
    initial.native_document = json!({"cotas":[{"x_mm":40.0},{"x_mm":80.0}]});
    initial.ensure_annotation_ids().unwrap();
    let first = initial.annotation_id(0, 0).unwrap();
    let second = initial.annotation_id(0, 1).unwrap();
    let mut edited = initial.clone();
    edited.native_document["cotas"][0]["x_mm"] = json!(53.0);
    let mut deleted = edited.clone();
    deleted.native_document["cotas"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    deleted.remove_annotation_id(0, 0);
    let mut restored = edited.clone();
    restored.retain_annotation_order(&deleted);
    assert_eq!(restored.annotation_ids[0], vec![second, first]);
    assert_eq!(
        restored.native_document["cotas"],
        json!([{"x_mm":80.0},{"x_mm":53.0}])
    );
    let mut undo_group = initial.clone();
    undo_group.retain_annotation_order(&restored);
    assert_eq!(undo_group.annotation_ids[0], vec![second, first]);
    assert_eq!(
        undo_group.native_document["cotas"],
        json!([{"x_mm":80.0},{"x_mm":40.0}])
    );
    edited.retain_annotation_order(&undo_group);
    assert_eq!(edited.annotation_ids[0], vec![second, first]);
    assert_eq!(edited.native_document, restored.native_document);
}

#[test]
fn equal_replacement_has_a_new_owner_and_add_undo_removes_only_that_owner() {
    let mut sheet = Sheet::default();
    sheet.native_document = json!({"cotas":[{"x_mm":40.0},{"x_mm":40.0}]});
    sheet.ensure_annotation_ids().unwrap();
    let original = sheet.annotation_id(0, 0).unwrap();
    let before = sheet.clone();
    let removed = sheet.native_document["cotas"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    sheet.remove_annotation_id(0, 0);
    sheet.native_document["cotas"]
        .as_array_mut()
        .unwrap()
        .push(removed);
    let new = sheet.push_annotation_id(0);
    assert_ne!(new, original);
    assert_eq!(sheet.annotation_index(0, original), None);
    let mut undo = before;
    undo.retain_annotation_order(&sheet);
    assert_eq!(undo.annotation_id(0, 1), Some(original));
    assert_eq!(undo.annotation_index(0, new), None);
}
