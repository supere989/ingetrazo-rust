//! Opt-in UI-thread engineering consumer; full Scene scripting remains incomplete.
//! Hydrates only once; external document changes refuse instead of discarding scope.
use ingetrazo_ai_interpreter_proof::Controller;
#[derive(Default)]
pub(crate) struct Engine {
    controller: Option<Controller>,
    revision: Option<u64>,
    #[cfg(test)]
    export_pressure: bool,
}
impl Engine {
    pub fn execute(
        &mut self,
        mesh: &ingetrazo_core::mesh::Mesh,
        revision: u64,
        code: &str,
    ) -> Result<
        (
            bool,
            Option<(
                ingetrazo_core::mesh::Mesh,
                ingetrazo_ai_interpreter_proof::owner_budget::Lease,
            )>,
        ),
        String,
    > {
        if self.revision.is_some_and(|r| r != revision) {
            return Err("Engineering Python scope needs external document/history synchronization; session retained".into());
        }
        if self.controller.is_none() {
            let mut controller = Controller::new();
            controller.import_native_mesh(mesh)?;
            controller.enable_history();
            self.controller = Some(controller);
            self.revision = Some(revision);
        }
        let controller = self.controller.as_mut().unwrap();
        let before = controller.native_mesh_revision()?;
        let changed = match controller.execute(code) {
            Ok(changed) => changed,
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
        // Temporary cold export. A prepared changed-row document journal must replace
        // this copy before production acceptance; no per-command scope hydration.
        let native = if native_changed {
            #[cfg(test)]
            if self.export_pressure {
                let budget = controller.budget();
                budget.set_limit(Some(budget.snapshot().total()));
            }
            match controller.native_mesh_snapshot_admitted() {
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
        Ok((changed, native))
    }
    pub fn published(&mut self, revision: u64) {
        self.revision = Some(revision);
    }
}
impl crate::IngeTrazoApp {
    #[doc(hidden)]
    pub fn engineering_python_budget(
        &self,
    ) -> Option<ingetrazo_ai_interpreter_proof::owner_budget::OwnerBudget> {
        self.engineering_python
            .controller
            .as_ref()
            .map(Controller::budget)
    }

    pub(crate) fn run_engineering_python(&mut self, code: &str) {
        if self.undo_stack.try_reserve(1).is_err() {
            self.console_logs
                .push("Engineering Python refused before execution: history capacity".into());
            return;
        }
        match self
            .engineering_python
            .execute(&self.scene.mesh, self.model_revision, code)
        {
            Ok((changed, native)) => {
                let published = native.is_some();
                if let Some((mesh, export_charge)) = native {
                    if changed {
                        self.save_undo_state();
                    } else {
                        self.bump_model_revision();
                    }
                    self.scene.mesh = mesh;
                    self.scene.bump_version();
                    self.engineering_python.published(self.model_revision);
                    drop(export_charge);
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
            }
            Err(error) => {
                self.status_message = error.clone();
                self.console_logs
                    .push(format!("Engineering Python refused: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
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
