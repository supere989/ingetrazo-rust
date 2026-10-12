//! Opt-in UI-thread engineering consumer; full Scene scripting remains incomplete.
//! Hydrates only once; external document changes refuse instead of discarding scope.
use ingetrazo_ai_interpreter_proof::{
    native_publication::{NativePublication, PublicationMetrics},
    vm_mesh::scene::GroupPublication,
    Controller,
};
#[derive(Default)]
pub(crate) struct Engine {
    controller: Option<Controller>,
    receipt: Option<ingetrazo_ai_interpreter_proof::vm_mesh::ExecutionReceipt>,
    revision: Option<u64>,
    session: Option<uuid::Uuid>,
    command_depth: Option<usize>,
    exports: usize,
    history_steps: usize,
    row_publications: usize,
    last_publication: Option<PublicationMetrics>,
    before_publication_probe: Option<fn()>,
    pending_groups: Option<GroupPublication>,
    last_group_packs: usize,
    #[cfg(test)]
    export_pressure: bool,
}
impl Engine {
    pub fn execute(
        &mut self,
        scene: &ingetrazo_core::scene::Scene,
        revision: u64,
        code: &str,
    ) -> Result<(bool, Option<NativePublication>, Option<GroupPublication>), String> {
        if self.revision.is_some_and(|r| r != revision) {
            return Err("Engineering Python scope needs external document/history synchronization; session retained".into());
        }
        if self.controller.is_none() {
            let mut controller = Controller::new();
            controller.import_native_scene(scene)?;
            controller.enable_history();
            self.session = Some(uuid::Uuid::new_v4());
            self.controller = Some(controller);
            self.revision = Some(revision);
        }
        let controller = self.controller.as_mut().unwrap();
        let before = controller.native_mesh_revision()?;
        self.receipt = None;
        let changed = match controller.execute_receipt(code) {
            Ok(receipt) => {
                let changed = receipt.changed;
                let execution_error = receipt.error.is_some();
                self.receipt = Some(receipt);
                if execution_error { return Ok((false, None, None)); }
                changed
            },
            Err(error) => {
                self.controller.take();
                self.revision = None;
                return Err(format!(
                    "Execution refused; interpreter scope retired: {error}"
                ));
            }
        };
        let native_changed = match controller.native_mesh_revision() {
            Ok(after) => before != after,
            Err(error) => {
                self.controller.take();
                self.revision = None;
                return Err(format!(
                    "Native publication refused; interpreter scope retired: {error}"
                ));
            }
        };
        // Callback-free changed rows only; existing application history still
        // retains one full before-Scene snapshot. No per-command hydration/export.
        let native = if native_changed {
            #[cfg(test)]
            if self.export_pressure {
                let budget = controller.budget();
                budget.set_limit(Some(budget.snapshot().total()));
            }
            match controller.native_mesh_publication(&scene.mesh) {
                Ok(native) => Some(native),
                Err(error) => {
                    self.controller.take();
                    self.revision = None;
                    return Err(format!(
                        "Native publication refused; interpreter scope retired: {error}"
                    ));
                }
            }
        } else {
            None
        };
        #[cfg(test)]
        if self.export_pressure {
            let budget = controller.budget();
            budget.set_limit(Some(budget.snapshot().total()));
        }
        let groups = match controller.native_groups_publication() {
            Ok(Some(receipt)) => {
                self.last_group_packs = receipt.packed_meshes;
                if receipt.groups != scene.groups
                    || receipt
                        .shares
                        .iter()
                        .any(|(id, mesh)| scene.mesh_shares.get(id) != Some(mesh))
                    || receipt.definitions.iter().any(|(id, definition)| {
                        scene.component_definitions.get(id) != Some(definition)
                    })
                {
                    Some(receipt)
                } else {
                    None
                }
            }
            Ok(_) => None,
            Err(error) => {
                self.controller.take();
                self.revision = None;
                return Err(format!(
                    "Group publication refused; interpreter scope retired: {error}"
                ));
            }
        };
        self.command_depth = if changed && (native.is_some() || groups.is_some()) {
            Some(controller.history_counts().0)
        } else {
            None
        };
        Ok((changed, native, groups))
    }
    fn command_marker(&self) -> Option<(uuid::Uuid, usize)> {
        Some((self.session?, self.command_depth?))
    }
    /// None means an unrelated/retired session owns this entry. Its VM remains
    /// unsynchronized and cannot execute further scripts; normal app history works.
    fn restore_owned(
        &mut self,
        marker: (uuid::Uuid, usize),
        revision: u64,
        redo: bool,
    ) -> Option<Result<(), String>> {
        if self.session != Some(marker.0) || self.revision != Some(revision) {
            return None;
        }
        Some((|| {
            let controller = self
                .controller
                .as_mut()
                .ok_or("Interpreter session retired")?;
            let depth = controller.history_counts().0;
            let expected = if redo {
                marker
                    .1
                    .checked_sub(1)
                    .ok_or("Invalid interpreter history depth")?
            } else {
                marker.1
            };
            if depth != expected {
                return Err("Interpreter/application chronological history mismatch".into());
            }
            if !(if redo {
                controller.redo()?
            } else {
                controller.undo()?
            }) {
                return Err("Interpreter command missing".into());
            }
            let groups = match controller.native_groups_publication() {
                Ok(groups) => {
                    self.last_group_packs = groups.as_ref().map_or(0, |g| g.packed_meshes);
                    groups
                }
                Err(error) => {
                    let restored = if redo {
                        controller.undo()
                    } else {
                        controller.redo()
                    };
                    if restored != Ok(true) {
                        self.controller.take();
                        self.revision = None;
                        return Err(format!(
                            "Group history publication refused; scope retired: {error}"
                        ));
                    }
                    return Err(format!(
                        "Group history publication refused; original history retained: {error}"
                    ));
                }
            };
            self.pending_groups = groups;
            self.history_steps += 1;
            Ok(())
        })())
    }
    pub fn published(&mut self, revision: u64) {
        self.revision = Some(revision);
    }
}
impl crate::IngeTrazoApp {
    #[doc(hidden)]
    pub fn engineering_python_history_observation(&self) -> Option<((usize, usize), usize, usize)> {
        self.engineering_python.controller.as_ref().map(|c| {
            (
                c.history_counts(),
                self.engineering_python.exports,
                self.engineering_python.history_steps,
            )
        })
    }
    #[doc(hidden)]
    pub fn engineering_python_group_pack_count(&self) -> usize {
        self.engineering_python.last_group_packs
    }
    #[doc(hidden)]
    pub fn engineering_python_publication_observation(
        &self,
    ) -> (usize, Option<PublicationMetrics>) {
        (
            self.engineering_python.row_publications,
            self.engineering_python.last_publication,
        )
    }
    /// Cold engineering failure-injection boundary, never exposed to scripts.
    /// The one-shot hook runs after the before-state is prepared, before the
    /// exclusive target ticket reserves application capacity. Default is absent.
    #[doc(hidden)]
    pub fn engineering_python_publication_probe(&mut self, hook: Option<fn()>) {
        self.engineering_python.before_publication_probe = hook;
    }
    #[doc(hidden)]
    pub fn engineering_python_budget(
        &self,
    ) -> Option<ingetrazo_ai_interpreter_proof::owner_budget::OwnerBudget> {
        self.engineering_python
            .controller
            .as_ref()
            .map(Controller::budget)
    }

    pub(crate) fn restore_engineering_python_history(
        &mut self,
        redo: bool,
    ) -> Result<bool, String> {
        let marker = (if redo {
            self.redo_stack.last()
        } else {
            self.undo_stack.last()
        })
        .and_then(|s| s.history_python);
        let Some(marker) = marker else {
            return Ok(false);
        };
        // Reserve the actual destination before stepping the persistent VM.
        if redo {
            self.undo_stack.try_reserve(1)
        } else {
            self.redo_stack.try_reserve(1)
        }
        .map_err(|_| "Interpreter application history capacity refused".to_string())?;
        let restored = self
            .engineering_python
            .restore_owned(marker, self.model_revision, redo);
        match restored {
            Some(Ok(())) => {
                let count = self
                    .engineering_python
                    .pending_groups
                    .as_ref()
                    .map_or(0, |g| g.shares.len());
                // Reserve the snapshot that will actually become visible, not the
                // current scene that app Undo/Redo is about to replace.
                let target = if redo {
                    self.redo_stack.last_mut()
                } else {
                    self.undo_stack.last_mut()
                };
                let definition_count = self
                    .engineering_python
                    .pending_groups
                    .as_ref()
                    .map_or(0, |g| g.definitions.len());
                let target = target.unwrap();
                if target.mesh_shares.try_reserve(count).is_err()
                    || target
                        .component_definitions
                        .try_reserve(definition_count)
                        .is_err()
                {
                    self.engineering_python.pending_groups.take();
                    self.engineering_python.controller.take();
                    self.engineering_python.revision = None;
                    return Err(
                        "Group share history capacity refused; interpreter scope retired".into(),
                    );
                }
                Ok(true)
            }
            Some(Err(error)) => Err(error),
            None => Ok(false),
        }
    }

    pub(crate) fn apply_engineering_python_history_groups(&mut self) {
        if let Some(groups) = self.engineering_python.pending_groups.take() {
            self.scene.mesh_shares.extend(groups.shares);
            self.scene.component_definitions.extend(groups.definitions);
            self.scene.groups = groups.groups;
            drop(groups.charge);
        }
    }
    pub(crate) fn run_engineering_python(&mut self, code: &str) {
        let _ = self.execute_engineering_python(code);
    }
    pub(crate) fn execute_engineering_python(&mut self, code: &str)
        -> Result<ingetrazo_ai_interpreter_proof::vm_mesh::ExecutionReceipt, String>
    {
        if self.undo_stack.try_reserve(1).is_err() {
            self.console_logs
                .push("Engineering Python refused before execution: history capacity".into());
            return Err("Engineering Python history capacity refused".into());
        }
        match self
            .engineering_python
            .execute(&self.scene, self.model_revision, code)
        {
            Ok((changed, native, groups)) => {
                let published = native.is_some() || groups.is_some();
                if published {
                    if let Some(receipt) = &groups {
                        if self
                            .scene
                            .mesh_shares
                            .try_reserve(receipt.shares.len())
                            .is_err()
                            || self
                                .scene
                                .component_definitions
                                .try_reserve(receipt.definitions.len())
                                .is_err()
                        {
                            self.engineering_python.controller.take();
                            self.engineering_python.revision = None;
                            self.status_message =
                                "Group shares publication refused; interpreter scope retired"
                                    .into();
                            self.console_logs.push(self.status_message.clone());
                            return Err(self.status_message.clone());
                        }
                    }
                    // One existing application Undo snapshot, including group membership.
                    let before = changed.then(|| self.scene.clone());
                    if let Some(probe) = self.engineering_python.before_publication_probe.take() {
                        probe();
                    }
                    let prepared = match native
                        .map(|publication| publication.prepare(&mut self.scene.mesh))
                        .transpose()
                    {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            self.engineering_python.controller.take();
                            self.engineering_python.revision = None;
                            self.status_message = format!(
                                "Native publication refused; interpreter scope retired: {error}"
                            );
                            self.console_logs.push(format!(
                                "Engineering Python refused: {}",
                                self.status_message
                            ));
                            return Err(self.status_message.clone());
                        }
                    };
                    if let Some(prepared) = prepared {
                        let metrics = prepared.publish();
                        self.engineering_python.row_publications += 1;
                        self.engineering_python.last_publication = Some(metrics);
                    }
                    if let Some(groups) = groups {
                        self.scene.mesh_shares.extend(groups.shares);
                        self.scene.component_definitions.extend(groups.definitions);
                        self.scene.groups = groups.groups;
                        drop(groups.charge);
                    }
                    if let Some(before) = before {
                        self.save_prepared_undo_state(before);
                        self.undo_stack.last_mut().unwrap().history_python =
                            self.engineering_python.command_marker();
                    } else {
                        self.bump_model_revision();
                    }
                    self.scene.bump_version();
                    self.engineering_python.published(self.model_revision);
                }
                self.console_logs.push(
                    if published && changed {
                        "Engineering Python mesh command committed"
                    } else if published {
                        "Engineering Python document updated without a new history command"
                    } else if changed {
                        "Engineering Python VM-only command completed; visible document unchanged"
                    } else {
                        "Engineering Python command completed without history change"
                    }
                    .into(),
                );
                let mut receipt = self.engineering_python.receipt.take().unwrap_or_default();
                receipt.changed = published && changed;
                if !receipt.stdout.is_empty() { self.console_logs.push(receipt.stdout.clone()); }
                if !receipt.stderr.is_empty() { self.console_logs.push(receipt.stderr.clone()); }
                Ok(receipt)
            }
            Err(error) => {
                self.status_message = error.clone();
                self.console_logs
                    .push(format!("Engineering Python refused: {error}"));
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    #[test]
    fn native_double_placement_refuses_before_unrelated_publication() {
        for x in [0.1, f64::from(f32::MAX) * 2.] {
            let mut app = crate::IngeTrazoApp::new();
            let mut group = ingetrazo_core::groups::Group::new("Native precision");
            group.transform = glam::DMat4::from_translation(glam::DVec3::new(x, 0., 0.));
            app.scene.groups.push(group);
            let before = app.scene.clone();
            app.engineering_python_enabled = true;
            app.run_script("mesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])");
            assert!(
                app.console_logs
                    .last()
                    .unwrap()
                    .contains("cannot be represented exactly"),
                "{:?}",
                app.console_logs
            );
            assert_eq!(app.scene, before);
            assert!(app.undo_stack.is_empty());
            assert!(app.redo_stack.is_empty());
        }
    }
    #[test]
    fn native_component_identity_hydration_and_refusal() {
        let mut app = crate::IngeTrazoApp::new();
        let group = ingetrazo_core::groups::Group::new("Native");
        let id = group.id;
        app.scene.groups.push(group);
        let definition = app.scene.make_component(id).unwrap();
        let mut second = app.scene.groups[0].clone();
        second.id = uuid::Uuid::new_v4();
        app.scene.groups.push(second);
        assert!(app.scene.groups[0].native_document.get("xform").is_none());
        app.engineering_python_enabled = true;
        app.run_script("a,b=groups; held=a.mesh; assert held is b.mesh; assert a.xform.isIdentity(); a.mesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])");
        assert!(
            !app.console_logs.last().unwrap().contains("refused"),
            "{:?}",
            app.console_logs
        );
        assert_eq!(app.scene.groups[0].component_definition, Some(definition));
        assert_eq!(app.scene.groups[1].component_definition, Some(definition));
        app.run_script("groups.append(Group(held,name='Third'))");
        assert_eq!(app.scene.groups.len(), 3);
        app.undo();
        assert_eq!(app.scene.groups.len(), 2);
        assert_eq!(app.scene.groups[0].mesh.faces.len(), 1);
        app.redo();
        assert_eq!(app.scene.groups.len(), 3);
        app.run_script("assert held is groups[0].mesh is groups[1].mesh is groups[2].mesh; assert len(held.faces)==1");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        drop(app);
        for stale in [false, true] {
            let mut app = crate::IngeTrazoApp::new();
            let group = ingetrazo_core::groups::Group::new("Native");
            let id = group.id;
            app.scene.groups.push(group);
            let definition = app.scene.make_component(id).unwrap();
            if stale {
                app.scene
                    .component_definitions
                    .get_mut(&definition)
                    .unwrap()
                    .mesh
                    .add_vertex(glam::DVec3::X);
            } else {
                app.scene.component_definitions.remove(&definition);
            }
            let before = app.scene.clone();
            app.engineering_python_enabled = true;
            app.run_script("groups[0].name='Wrong'");
            assert_eq!(app.scene, before);
            assert!(app.console_logs.last().unwrap().contains("refused"));
            assert!(app.undo_stack.is_empty());
        }
    }
    #[test]
    fn original_igz_definition_snapshot_admits_shared_owner() {
        let path =
            std::env::temp_dir().join(format!("component-codec-{}.igz", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            include_bytes!("../../../docs/audits/ai-console-imported-mesh/original-input.igz"),
        )
        .unwrap();
        let mut app = crate::IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let definition = app.scene.groups[0].component_definition.unwrap();
        let share = app.scene.groups[0].mesh_share.unwrap();
        assert_eq!(
            app.scene.component_definitions[&definition].mesh_share,
            None
        );
        app.engineering_python_enabled = true;
        app.run_script("a,b=groups; assert a.mesh is b.mesh; assert a.xform.isIdentity()");
        assert!(
            !app.console_logs.last().unwrap().contains("refused"),
            "{:?}",
            app.console_logs
        );
        assert_eq!(app.scene.groups[0].component_definition, Some(definition));
        assert_eq!(app.scene.groups[0].mesh_share, Some(share));
        assert_eq!(
            app.scene.component_definitions[&definition].mesh_share,
            Some(share)
        );
    }
    #[test]
    fn imported_mesh_reassignment_refusal_preserves_registry_and_redo() {
        let path = std::env::temp_dir().join(format!(
            "ingetrazo-imported-mesh-{}.igz",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            include_bytes!("../../../docs/audits/ai-console-imported-mesh/input.igz"),
        )
        .unwrap();
        let mut app = crate::IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        app.engineering_python_enabled = true;
        app.run_script(
            "a,b=groups; old=a.mesh; assert old is b.mesh; groups.append(Group(old,name='C'))",
        );
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        app.undo();
        assert_eq!(app.scene.groups.len(), 2);
        let before = app.scene.clone();
        let budget = app.engineering_python_budget().unwrap();
        app.engineering_python.export_pressure = true;
        app.run_script("a.mesh=Mesh()");
        assert_eq!(app.scene, before);
        assert_eq!(app.redo_stack.len(), 1);
        assert!(app.status_message.contains("scope retired"));
        assert_eq!(budget.snapshot().total(), 0);
        app.engineering_python.export_pressure = false;
        app.redo();
        assert_eq!(app.scene.groups.len(), 3);
        app.run_script("assert groups[0].mesh is groups[1].mesh is groups[2].mesh; groups[0].mesh=Mesh(); assert groups[1].mesh is groups[2].mesh and groups[0].mesh is not groups[1].mesh");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        assert_ne!(
            app.scene.groups[0].mesh_share,
            app.scene.groups[1].mesh_share
        );
        let a = &app.scene.groups[0];
        assert_eq!(a.component_definition, a.mesh_share);
        assert_eq!(
            app.scene.component_definitions[&a.component_definition.unwrap()].mesh_share,
            a.mesh_share
        );
        assert_eq!(app.scene.mesh_shares[&a.mesh_share.unwrap()], a.mesh);
    }
    #[test]
    fn recipe_caught_native_orientation_refusal_aborts_visible_command() {
        let mut app = crate::IngeTrazoApp::new();
        app.engineering_python_enabled = true;
        app.run_script("assert model.groups is groups");
        let budget = app.engineering_python_budget().unwrap();
        let before = app.scene.clone();
        app.run_script("g=extrude([(0,0),(1,0),(1,1),(0,1)],0,1,name='Candidate')\ng.mesh.faces[0].attrs['back']={'color':(1,0,0)}\nfrom console_scene import orient_owned\ntry:\n orient_owned(g.mesh)\nexcept ValueError:\n pass");
        assert_eq!(app.scene, before);
        assert!(app.undo_stack.is_empty());
        assert!(app.redo_stack.is_empty());
        assert!(app.status_message.contains("scope retired"));
        assert!(app.engineering_python.controller.is_none());
        assert_eq!(budget.snapshot().total(), 0);
    }
    #[test]
    fn group_publication_refusal_retains_document_redo_and_retries() {
        let mut app = crate::IngeTrazoApp::new();
        app.engineering_python_enabled = true;
        app.run_script("groups.append(Group(name='First'))");
        assert_eq!(app.scene.groups.len(), 1);
        app.undo();
        assert!(app.scene.groups.is_empty());
        assert_eq!(app.redo_stack.len(), 1);
        let before = app.scene.clone();
        app.engineering_python.export_pressure = true;
        app.run_script("groups.append(Group(name='Refused'))");
        assert_eq!(app.scene, before);
        assert!(app.scene.groups.is_empty());
        assert_eq!(app.redo_stack.len(), 1);
        assert!(app.undo_stack.is_empty());
        assert!(app.status_message.contains("scope retired"));
        assert!(app.engineering_python.controller.is_none());
        app.redo();
        assert_eq!(app.scene.groups[0].name, "First");
        app.engineering_python.export_pressure = false;
        app.run_script("groups.append(Group(name='Retry'))");
        assert_eq!(app.scene.groups.len(), 2);
    }
    #[test]
    fn nested_group_publication_admission_preserves_redo_and_retries() {
        let mut app = crate::IngeTrazoApp::new();
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/audits/ai-console-nested-scene/input.igz");
        app.scene = ingetrazo_formats::igz::load_igz(&input).unwrap();
        app.engineering_python_enabled = true;
        app.run_script("groups.append(Group(name='History'))");
        assert!(!app.status_message.contains("refused"));
        app.undo();
        let before = app.scene.clone();
        let undo = app.undo_stack.clone();
        let redo = app.redo_stack.clone();
        app.engineering_python.export_pressure = true;
        app.run_script("groups[0].children[0].children[0].xform.translate(.125,0,0); groups.append(Group(name='Refused'))");
        assert_eq!(app.scene, before);
        assert_eq!(app.undo_stack, undo);
        assert_eq!(app.redo_stack, redo);
        assert!(app.engineering_python.controller.is_none());
        assert!(app.status_message.contains("scope retired"));
        app.redo();
        assert_eq!(app.scene.groups.last().unwrap().name, "History");
        app.engineering_python.export_pressure = false;
        app.run_script("groups[0].children[0].children[0].xform.translate(.125,0,0)");
        assert!(!app.status_message.contains("refused"));
        assert_eq!(
            app.scene.groups[0].children[0].children[0]
                .transform
                .w_axis
                .x,
            0.625
        );
    }
    #[test]
    fn changed_row_prepare_refuses_stale_unchanged_coordinate_before_writes() {
        let mut target = ingetrazo_core::mesh::Mesh::new();
        target.add_vertex(glam::DVec3::ZERO);
        target.add_vertex(glam::DVec3::X);
        let mut controller = ingetrazo_ai_interpreter_proof::Controller::new();
        controller.import_native_mesh(&target).unwrap();
        controller.enable_history();
        controller
            .execute("mesh.vertices[0].position.setZ(2)")
            .unwrap();
        let budget = controller.budget();
        let charged = budget.snapshot().total();
        let publication = controller.native_mesh_publication(&target).unwrap();
        assert!(budget.snapshot().total() > charged);
        target.vertices[1].position = glam::DVec3::Y;
        let before = target.clone();
        assert!(publication.prepare(&mut target).is_err());
        assert_eq!(target, before);
        assert_eq!(budget.snapshot().total(), charged);
    }
    #[test]
    fn changed_row_topology_and_unrelated_document_state_share_one_command() {
        let mut app = crate::IngeTrazoApp::new();
        let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(ids, vec![]).unwrap();
        let layers = app.scene.layers.clone();
        let images = app.scene.image_planes.clone();
        let style = app.scene.display_style.clone();
        app.engineering_python_enabled = true;
        app.run_script("held=mesh.faces[0]; added=mesh.add_face([QVector3D(3,0,0),QVector3D(4,0,0),QVector3D(3,1,0)]); held.attrs['mat']='First'");
        assert_eq!(app.scene.mesh.faces.len(), 2);
        assert_eq!(app.scene.mesh.faces[0].attrs["mat"], "First");
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.scene.layers, layers);
        assert_eq!(app.scene.image_planes, images);
        assert_eq!(app.scene.display_style, style);
        let before = app.scene.mesh.clone();
        app.run_script("mesh.remove_face(held)");
        assert_eq!(app.scene.mesh.faces.len(), 1);
        assert_eq!(app.undo_stack.len(), 2);
        app.undo();
        assert_eq!(app.scene.mesh, before);
        app.run_script("assert held is mesh.faces[0]; assert added is mesh.faces[1]");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        app.redo();
        assert_eq!(app.scene.mesh.faces.len(), 1);
        app.run_script("assert held not in mesh.faces; assert added is mesh.faces[0]");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
    }
    #[test]
    fn changed_row_consumer_keeps_unchanged_payload_and_index_correct() {
        let mut app = crate::IngeTrazoApp::new();
        for x in 0..128 {
            let x = x as f64 * 3.;
            let ids = [
                glam::DVec3::new(x, 0., 0.),
                glam::DVec3::new(x + 1., 0., 0.),
                glam::DVec3::new(x, 1., 0.),
            ]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
            app.scene.mesh.add_face(ids, vec![]).unwrap();
        }
        app.scene.mesh.faces.last_mut().unwrap().attrs.insert(
            "vendor".into(),
            serde_json::json!({"note":"unchanged payload"}),
        );
        let pointer = app.scene.mesh.faces.last().unwrap().attrs["vendor"]["note"]
            .as_str()
            .unwrap()
            .as_ptr();
        app.engineering_python_enabled = true;
        app.run_script("held=mesh.faces[0]; held.attrs['mat']='First'");
        let m = app.engineering_python.last_publication.unwrap();
        assert_eq!(
            (m.vertex_rows, m.edge_rows, m.face_rows, m.index_vertices),
            (0, 0, 1, 0)
        );
        assert_eq!(
            app.scene.mesh.faces.last().unwrap().attrs["vendor"]["note"]
                .as_str()
                .unwrap()
                .as_ptr(),
            pointer
        );
        assert_eq!(app.engineering_python.exports, 0);
        assert_eq!(app.undo_stack.len(), 1);
        app.run_script("held.loop[0].position.setZ(2)");
        let m = app.engineering_python.last_publication.unwrap();
        assert_eq!(
            (m.vertex_rows, m.edge_rows, m.face_rows, m.index_vertices),
            (1, 0, 0, 384)
        );
        assert_eq!(app.scene.mesh.add_vertex(glam::DVec3::new(0., 0., 2.)), 0);
        assert_eq!(
            app.scene.mesh.faces.last().unwrap().attrs["vendor"]["note"]
                .as_str()
                .unwrap()
                .as_ptr(),
            pointer
        );
        assert_eq!(app.undo_stack.len(), 2);
        app.undo();
        app.redo();
        app.run_script("assert held is mesh.faces[0]; assert held.loop[0].position.z()==2");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
    }
    #[test]
    fn refused_vm_history_preserves_visible_command_and_retry() {
        let mut app = crate::IngeTrazoApp::new();
        let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(ids, vec![]).unwrap();
        app.engineering_python_enabled = true;
        app.run_script("held=mesh.faces[0]; held.attrs['mat']='Edited'");
        let mesh = app.scene.mesh.clone();
        let marker = app.undo_stack.last().unwrap().history_python;
        let budget = app.engineering_python_budget().unwrap();
        let charged = budget.snapshot().total();
        budget.set_limit(Some(charged));
        app.undo();
        assert_eq!(app.scene.mesh, mesh);
        assert_eq!(app.undo_stack.len(), 1);
        assert!(app.redo_stack.is_empty());
        assert_eq!(app.undo_stack.last().unwrap().history_python, marker);
        assert_eq!(budget.snapshot().total(), charged);
        assert_eq!(
            app.engineering_python
                .controller
                .as_ref()
                .unwrap()
                .history_counts(),
            (1, 0)
        );
        budget.set_limit(None);
        app.undo();
        app.run_script("assert held is mesh.faces[0]; assert 'mat' not in held.attrs");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        let undone = app.scene.mesh.clone();
        let charged = budget.snapshot().total();
        budget.set_limit(Some(charged));
        app.redo();
        assert_eq!(app.scene.mesh, undone);
        assert_eq!(app.undo_stack.len(), 0);
        assert_eq!(app.redo_stack.len(), 1);
        assert_eq!(budget.snapshot().total(), charged);
        assert_eq!(
            app.engineering_python
                .controller
                .as_ref()
                .unwrap()
                .history_counts(),
            (0, 1)
        );
        budget.set_limit(None);
        app.redo();
        app.run_script("assert held is mesh.faces[0]; assert held.attrs['mat']=='Edited'");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
    }
    #[test]
    fn application_undo_redo_retains_real_vm_aliases_and_globals() {
        let mut app = crate::IngeTrazoApp::new();
        let vertices = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(vertices, vec![]).unwrap();
        app.engineering_python_enabled = true;
        app.run_script("held=mesh.faces[0]; held_vertex=held.loop[0]; token=41; held.attrs['mat']='First'; held_vertex.position.setZ(2)");
        app.run_script("held.attrs['mat']='Second'; token+=1");
        assert_eq!(app.undo_stack.len(), 2);
        app.undo();
        assert_eq!(app.scene.mesh.faces[0].attrs["mat"], "First");
        app.run_script("assert token==42; assert held is mesh.faces[0]; assert held_vertex is held.loop[0]; assert held.attrs['mat']=='First'");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        assert_eq!(app.redo_stack.len(), 1);
        app.redo();
        app.run_script(
            "assert token==42; assert held is mesh.faces[0]; assert held.attrs['mat']=='Second'",
        );
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        app.undo();
        app.undo();
        app.run_script("assert token==42; assert held is mesh.faces[0]; assert held_vertex.position.z()==0; assert 'mat' not in held.attrs");
        assert!(!app.console_logs.last().unwrap().contains("refused"));
        app.redo();
        app.run_script("held.attrs['mat']='Branch'");
        assert!(app.redo_stack.is_empty());
        assert_eq!(app.scene.mesh.faces[0].attrs["mat"], "Branch");
        app.undo();
        app.redo();
        app.run_script(
            "assert held is mesh.faces[0]; assert held.attrs['mat']=='Branch'; assert token==42",
        );
        assert!(!app.console_logs.last().unwrap().contains("refused"));
    }
    #[test]
    fn vm_only_mesh_creation_does_not_claim_document_commit() {
        let mut app = crate::IngeTrazoApp::new();
        let before = app.scene.mesh.clone();
        app.engineering_python_enabled = true;
        app.run_script("g=Mesh(); g.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])");
        assert_eq!(app.scene.mesh, before);
        assert_eq!(app.undo_stack.len(), 0);
        assert_eq!(app.redo_stack.len(), 0);
        assert!(!app.console_logs.last().unwrap().contains("committed"));
        app.run_script("assert len(g.faces)==1");
        assert_eq!(app.undo_stack.len(), 0);
        assert!(!app.console_logs.last().unwrap().contains("refused"));
    }
    #[test]
    fn post_execution_export_budget_refusal_retires_scope_preserves_document_and_history() {
        let mut app = crate::IngeTrazoApp::new();
        let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(ids, vec![]).unwrap();
        app.engineering_python_enabled = true;
        app.run_script("mesh.faces[0].attrs['mat']='First'");
        app.undo();
        let before = app.scene.mesh.clone();
        let redo = app.redo_stack.len();
        app.engineering_python = Engine::default();
        app.engineering_python.export_pressure = true;
        app.run_script("mesh.faces[0].attrs['mat']='Refused'");
        assert_eq!(app.scene.mesh, before);
        assert_eq!(app.redo_stack.len(), redo);
        assert_eq!(app.undo_stack.len(), 0);
        assert!(app.status_message.contains("scope retired"));
        assert!(app.engineering_python.controller.is_none());
        app.engineering_python.export_pressure = false;
        app.run_script("mesh.faces[0].attrs['mat']='Retry'");
        assert_eq!(app.scene.mesh.faces[0].attrs["mat"], "Retry");
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.redo_stack.len(), 0);
    }
    #[test]
    fn actual_console_current_mesh_persistent_scope_error_and_document_undo() {
        let mut app = crate::IngeTrazoApp::new();
        let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
            .into_iter()
            .map(|p| app.scene.mesh.add_vertex(p))
            .collect();
        app.scene.mesh.add_face(ids, vec![]).unwrap();
        let before = app.scene.mesh.clone();
        let layers = app.scene.layers.clone();
        app.engineering_python_enabled = true;
        app.run_script(
            "token=41; f=mesh.faces[0]; f.attrs['mat']='First'; f.loop[0].position.setZ(2)",
        );
        assert_eq!(app.scene.mesh.vertices[0].position.z, 2.);
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.scene.layers, layers);
        app.run_script(
            "token+=1; assert token==42; assert f is mesh.faces[0]; f.attrs['mat']='Second'",
        );
        assert_eq!(app.scene.mesh.faces[0].attrs["mat"], "Second");
        assert_eq!(app.undo_stack.len(), 2);
        let edited = app.scene.mesh.clone();
        app.run_script("f.loop[0].position.setZ(99); raise ValueError('refuse')");
        assert_eq!(app.scene.mesh, edited);
        assert_eq!(app.undo_stack.len(), 2);
        assert!(app
            .console_logs
            .last()
            .unwrap()
            .starts_with("Engineering Python refused:"));
        app.run_script("inspection=1");
        assert_eq!(app.undo_stack.len(), 2);
        assert_eq!(app.scene.mesh, edited);
        app.undo();
        assert_eq!(app.scene.mesh.faces[0].attrs["mat"], "First");
        app.undo();
        assert_eq!(app.scene.mesh, before);
        assert_eq!(app.redo_stack.len(), 2);
        app.run_script("mesh.vertices.clear()");
        assert_eq!(app.scene.mesh, before);
        assert_eq!(app.redo_stack.len(), 2);
        assert!(app
            .status_message
            .contains("external document/history synchronization"));
    }
}
