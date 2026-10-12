//! Borrowed parked edit-context geometry. No document/history publication.
use crate::{IngeTrazoApp, component_edit::ComponentEditSession};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestMode {
    Normal,
    Fade,
    Hide,
}

pub(crate) struct ContextView<'a> {
    active: &'a Scene,
    stack: &'a [ComponentEditSession],
    mode: RestMode,
}
impl<'a> ContextView<'a> {
    pub(crate) fn new(app: &'a IngeTrazoApp) -> Self {
        let mode = match app
            .document_session
            .preferences
            .extra
            .get("display/edit_rest_mode")
            .and_then(serde_json::Value::as_str)
        {
            Some("normal") => RestMode::Normal,
            Some("hide") => RestMode::Hide,
            _ => RestMode::Fade,
        };
        Self {
            active: &app.scene,
            stack: &app.component_edit_stack,
            mode,
        }
    }
    pub(crate) fn mode(&self) -> RestMode {
        self.mode
    }
    pub(crate) fn is_editing(&self) -> bool {
        !self.stack.is_empty()
    }
    /// RGB wash after lighting/texturing; never an opacity multiplier.
    pub(crate) fn context_fade(&self) -> f32 {
        if self.is_editing() && self.mode == RestMode::Fade {
            0.75
        } else {
            0.
        }
    }
    pub(crate) fn active_scene(&self) -> &'a Scene {
        self.active
    }
    /// Every parked loose mesh is already in the world coordinates used by
    /// begin_component_edit. Sibling groups retain their existing placements;
    /// use the ordinary recursive group/material/topology collector on them.
    /// Each edited container is excluded once: the next level supplies its
    /// live world mesh and siblings, and the final active Scene is the subject.
    /// Order is root surroundings first, then successively nested surroundings.
    pub(crate) fn visit_context(
        &self,
        mut mesh: impl FnMut(&'a Mesh),
        mut group: impl FnMut(&'a Group),
    ) {
        if self.mode == RestMode::Hide {
            return;
        }
        for session in self.stack {
            let owner = session.parked_scene();
            mesh(&owner.mesh);
            for sibling in &owner.groups {
                if sibling.id != session.edited_group_id() {
                    group(sibling);
                }
            }
        }
    }
}
