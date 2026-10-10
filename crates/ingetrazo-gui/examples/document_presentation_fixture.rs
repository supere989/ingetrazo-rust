use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_render::style::VisualStyle;
use serde_json::json;
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let out = std::path::PathBuf::from(std::env::args().nth(2).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", out.join("config"));
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.open_document(&dir.join("input.igz"));
    let state = |a: &IngeTrazoApp| json!({"face_mode":match a.style.visual_style{VisualStyle::Wireframe=>"wireframe",VisualStyle::HiddenLine=>"hidden_line",VisualStyle::Monochrome=>"monochrome",VisualStyle::XRay=>"xray",_=>if a.style.show_textures{"textures"}else{"shaded"}},"edges":a.style.show_edges,"profiles":a.style.show_profiles,"back_edges":a.style.back_edges_hidden,"edge_color":&a.style.edge_color[..3],"front_color":&a.style.front_color[..3],"background":&a.style.background_color[..3],"sky":a.style.show_sky,"section_fill":a.scene.show_section_fill,"shadow_enabled":a.shadows_enabled,"undo":a.undo_stack.len(),"redo":a.redo_stack.len()});
    let mut states = vec![state(&a)];
    a.save_undo_state();
    a.scene.mesh.vertices[1].position.x += 2.;
    a.scene.bump_version();
    a.style.visual_style = VisualStyle::Wireframe;
    a.style.back_edges_hidden = false;
    a.shadows_enabled = false;
    states.push(state(&a));
    a.undo();
    states.push(state(&a));
    a.redo();
    states.push(state(&a));
    a.add_saved_view("Scene 1".into());
    a.save_to_path(&out.join("edited.igz"));
    let scene = ingetrazo_formats::igz::load_igz(&out.join("edited.igz")).unwrap();
    let view = &scene.saved_views[0];
    let report = json!({"states":states,"saved_raw_style":scene.native_document.get("display_style"),"saved_raw_shadows":scene.native_document.get("shadows"),"view_style":view.style,"view_shadows":view.shadows});
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
