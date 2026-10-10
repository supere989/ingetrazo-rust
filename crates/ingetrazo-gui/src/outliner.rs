//! Hierarchical group selection and undoable placement properties.
use crate::IngeTrazoApp;use ingetrazo_core::{groups::Group,scene::SelectionItem};use uuid::Uuid;
fn path(groups:&[Group],id:Uuid)->Option<Vec<usize>>{for(i,g)in groups.iter().enumerate(){if g.id==id{return Some(vec![i]);}if let Some(mut child)=path(&g.children,id){child.insert(0,i);return Some(child);}}None}
fn group_mut(groups:&mut [Group],id:Uuid)->Option<&mut Group>{for g in groups{if g.id==id{return Some(g);}if let Some(found)=group_mut(&mut g.children,id){return Some(found);}}None}
impl IngeTrazoApp {
    /// Nested rows open their ancestors, then select the actual child in that
    /// editing context. Geometry tools therefore operate on the named part.
    pub fn select_outliner_group(&mut self,id:Uuid,extend:bool)->bool {
        let Some(path)=path(&self.scene.groups,id) else{return false;};
        let mut hidden=false;let mut groups=&self.scene.groups;for i in &path{let g=&groups[*i];hidden|=g.hidden;groups=&g.children;}if hidden{return false;}
        for &index in path.iter().take(path.len()-1){self.scene.clear_selection();self.scene.select(SelectionItem::Group(index));let depth=self.component_edit_stack.len();self.begin_component_edit();if self.component_edit_stack.len()==depth{return false;}}
        let index=*path.last().unwrap();if !extend||path.len()>1{self.scene.clear_selection();}self.scene.select(SelectionItem::Group(index));true
    }
    pub fn set_outliner_visibility(&mut self,id:Uuid,visible:bool){
        let Some(path)=path(&self.scene.groups,id)else{return;};let mut groups=&self.scene.groups;let mut current=None;for i in path{let g=&groups[i];current=Some(g.hidden);groups=&g.children;}if current==Some(!visible){return;}
        self.save_undo_state();group_mut(&mut self.scene.groups,id).unwrap().hidden=!visible;self.scene.bump_version();
    }
    pub fn rename_outliner_group(&mut self,id:Uuid,name:String){
        if name.trim().is_empty(){return;}let Some(g)=group_mut(&mut self.scene.groups,id)else{return;};if g.name==name{return;}
        self.save_undo_state();group_mut(&mut self.scene.groups,id).unwrap().name=name;self.scene.bump_version();
    }
}
#[derive(Clone)]enum Action{Select(Uuid,bool),Edit(Uuid),Visibility(Uuid,bool),Rename(Uuid,String),Unique(Uuid),Explode(Uuid)}
pub fn render_outliner(app:&mut IngeTrazoApp,ui:&mut egui::Ui){
    fn rows(groups:&[Group],selected:&std::collections::HashSet<SelectionItem>,root:bool,ui:&mut egui::Ui,actions:&mut Vec<Action>){
        for(i,group)in groups.iter().enumerate(){ui.push_id(group.id,|ui|{
            ui.horizontal(|ui|{
                let mut shown=!group.hidden;if ui.checkbox(&mut shown,"").changed(){actions.push(Action::Visibility(group.id,shown));}
                let label=if group.component_definition.is_some(){format!("◈ {}",group.name)}else{group.name.clone()};
                let response=ui.selectable_label(root&&selected.contains(&SelectionItem::Group(i)),label);
                if response.clicked(){actions.push(Action::Select(group.id,ui.input(|i|i.modifiers.ctrl||i.modifiers.command||i.modifiers.shift)));}
                if response.double_clicked(){actions.push(Action::Edit(group.id));}
                response.context_menu(|ui|{
                    if ui.button("Edit group").clicked(){actions.push(Action::Edit(group.id));ui.close_menu();}
                    if ui.button("Rename").clicked(){actions.push(Action::Rename(group.id,group.name.clone()));ui.close_menu();}
                    if ui.button(if group.hidden{"Show"}else{"Hide"}).clicked(){actions.push(Action::Visibility(group.id,group.hidden));ui.close_menu();}
                    if group.component_definition.is_some()&&ui.button("Make Unique").clicked(){actions.push(Action::Unique(group.id));ui.close_menu();}
                    if ui.button("Explode").clicked(){actions.push(Action::Explode(group.id));ui.close_menu();}
                });
            });
            if !group.children.is_empty(){egui::CollapsingHeader::new(format!("{} parts",group.children.len())).id_salt((group.id,"parts")).default_open(true).show(ui,|ui|rows(&group.children,selected,false,ui,actions));}
        });}
    }
    egui::CollapsingHeader::new("Outliner / Parts").default_open(true).show(ui,|ui|{
        if !app.component_edit_stack.is_empty(){ui.label(format!("Editing depth {}",app.component_edit_stack.len()));ui.horizontal(|ui|{if ui.button("Finish").clicked(){app.finish_component_edit();}if ui.button("Cancel edit").clicked(){app.cancel_component_edit();}});}
        let mut actions=vec![];rows(&app.scene.groups,&app.scene.selection,true,ui,&mut actions);
        for action in actions{match action{
            Action::Select(id,extend)=>{app.select_outliner_group(id,extend);},Action::Visibility(id,visible)=>app.set_outliner_visibility(id,visible),
            Action::Rename(id,name)=>app.outliner_rename=Some((id,name)),
            Action::Edit(id)=>{if app.select_outliner_group(id,false){app.begin_component_edit();}},
            Action::Unique(id)=>{if app.select_outliner_group(id,false){app.make_component_unique();}},
            Action::Explode(id)=>{if app.select_outliner_group(id,false){app.explode_group();}},
        }}
        if let Some((id,mut name))=app.outliner_rename.clone(){ui.horizontal(|ui|{ui.text_edit_singleline(&mut name);if ui.button("Rename").clicked(){app.rename_outliner_group(id,name.clone());app.outliner_rename=None;}else if ui.button("Cancel").clicked(){app.outliner_rename=None;}else{app.outliner_rename=Some((id,name.clone()));}});}
    });
}
#[cfg(test)]mod tests{use super::*;use glam::{DMat4,DVec3};use ingetrazo_core::mesh::Mesh;
    #[test]fn nested_selection_opens_correct_context_and_properties_roundtrip_with_undo(){
        let mut app=IngeTrazoApp::new();let mut mesh=Mesh::new();mesh.add_vertex(DVec3::new(1.,2.,3.));let leaf=Group::with_mesh("leaf",mesh);let id=leaf.id;
        let mut middle=Group::new("middle");middle.transform=DMat4::from_translation(DVec3::X);middle.children.push(leaf);
        let mut outer=Group::new("outer");outer.children.push(middle);app.scene.groups.push(outer);
        let before=app.scene.clone();app.set_outliner_visibility(id,false);assert!(!app.select_outliner_group(id,false));app.undo();assert_eq!(app.scene,before);
        app.rename_outliner_group(id,"renamed".into());assert_eq!(app.scene.groups[0].children[0].children[0].name,"renamed");app.undo();assert_eq!(app.scene,before);
        assert!(app.select_outliner_group(id,false));assert_eq!(app.component_edit_stack.len(),2);assert_eq!(app.scene.groups[0].id,id);assert!(app.scene.selection.contains(&SelectionItem::Group(0)));
        app.rename_outliner_group(id,"inside".into());app.finish_component_edit();app.finish_component_edit();assert_eq!(app.scene.groups[0].children[0].children[0].name,"inside");app.undo();assert_eq!(app.scene.groups[0].children[0].children[0].name,"leaf");
    }
}
