# Mesh prototype identity contract

`Group.mesh_share: Option<Uuid>` is a Mesh prototype identity independent of
`component_definition`, `definition_member`, component flags, placement matrices
and placement child lists. `Scene.mesh_shares: HashMap<Uuid, Mesh>` retains the
complete prototype Mesh including vertex IDs, edge flags/curve metadata, face
attributes and UVW data. Both fields have serde defaults for old recovery data.

Native codec contract: the same original `proto` means the same mesh_share token,
including `component=false` placements. Different native prototypes never merge
merely because their geometry compares equal. `register_mesh_share` refuses a
conflicting existing token. Component construction assigns explicit fresh tokens
to authored meshes; repeated placements retain them. Make Unique detaches tokens.

`publish_mesh_share(token, mesh)` validates finite positions and vertex references
before changing anything, updates all existing linked placements and cached
component-definition meshes, and bumps model version. It preserves every child
list, placement UID, matrix and native per-placement payload. Publication permits
open/wire meshes; it does not incorrectly demand a closed solid. GUI history
comparison includes registry changes, and local edit scenes carry the registry.

This is the schema/API stage. Component Finish still uses the old template
replacement path and native IGZ has not yet mapped proto identities. Those
remaining stages must use publication rather than wholesale sibling child-list
replacement. The field alone is not native round-trip or full workflow proof.

Evidence: `component-child-list-source.json` and its actual source-command probe
prove selected child counts 2/4/0 for copy/array/delete while sibling count stays
1 with its UID and matrix unchanged. Placed component-false child copies share
Mesh; classic unplaced copies do not. `mesh_share_acceptance` independently tests
unequal placement topology, matrix/payload preservation, invalid atomic refusal,
equal-but-independent meshes, Make Unique detachment and recovery identity.
