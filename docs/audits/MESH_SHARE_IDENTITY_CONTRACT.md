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

## Component Finish followup

Finish now assigns only the selected container's authored child list and
publishes changed explicit mesh tokens separately. Sibling placement topology
is retained. Source-command controls and the GUI Move dispatcher now both yield
selected child counts 2/4/0 for copy/array/delete, sibling count 1, and exact
sibling Group preservation. Document Undo and Redo restore Groups and the mesh
registry; existing nested leaf mesh edit tests continue propagating shared mesh
changes in one outer history entry. Template placement matrices remain baseline
metadata rather than being rewritten to the edited instance's matrix.

This followup is tested on explicit identities produced by Make Component.
Native-proto decoding and migration of older tokenless component recovery data
remain required codec/compatibility followups. Actual rendered child-list exit
proof remains pending; earlier rendered Move-only Finish evidence is retained.

## Recovery and private own-mesh followup

`ComponentDefinition.mesh_share: Option<Uuid>` identifies its cached root Mesh;
publication updates that cache only by explicit token rather than inferring a
prototype from component flags. Minimal constructor defaults in other modules
are schema compatibility edits, not codec behavior changes.

`Scene.migrate_legacy_mesh_shares()` atomically restores old component recovery
identities from existing definition IDs and child-member paths. Cache geometry
is verified within that identity; equal unrelated geometry stays independent.
Divergent old placement caches receive private tokens. Repeated migration is
idempotent; missing definitions fail without changing the document. GUI edit
entry invokes migration before capturing the local session. Recovery/legacy
codec callers should invoke this public method after deserialization as well.

Actual original Scene entry proves nonidentity + nonempty own Mesh is copied
while its sibling keeps the prototype. Native edit sessions allocate a fresh
private token for that selected container, including unchanged Finish; Cancel
restores the original session snapshot. Empty own Mesh and identity placement
keep sharing. Frozen `component-own-mesh-source.json` includes four source
controls. GUI controls verify private-token selection, untouched sibling and
cached definition, and Undo/Redo. These controls use direct numeric Mesh edits;
world-space authoring into a transformed empty container still needs a distinct
coordinate-equivalence control because original entry bakes the parent matrix
into children and clears it, whereas native local editing retains that frame.
No full transformed-empty authoring or rendered own-Mesh parity is claimed.

## World-coordinate authoring correction

Fresh original `AddFaceCommand` root and nested controls author the same frozen
WORLD points into initially empty transformed containers. The original bakes
its container frame into child placements, enters world coordinates, and keeps
the empty own Mesh prototype shared. The earlier native local-frame publication
failed the sibling's world surfaces; `component-world-authoring-baseline-failure.log`
retains that failure on production `6880f46` with the new source-backed checker.

Native preview stays local. Finish now transforms the own Mesh and child matrices
into the original world frame, expresses the edited container as inverse enclosing
edit frame, and preserves world axes metadata. Existing nonempty own Mesh uses a
private token; empty own Mesh remains shared as in source. UVW uses the existing
affine covector transformation, vertex UIDs and opaque attrs stay on their entities.
Dimensions/sections still convert from the ORIGINAL edit placement rather than the
new baked placement, avoiding a coordinate regression.

The frozen source recipe and GUI Line dispatcher check root and nested own-Mesh
WORLD vertices, both shared siblings, unchanged existing child WORLD vertices,
prototype-token equality, one outer Undo/Redo entry, and registry restoration.
Prior four own-Mesh controls now use inverse-native-frame inputs for the same
world numeric points instead of conflating world and local input recipes.
This is bounded command/controller proof. Rendered nested authoring, reentry axes,
material/texture image comparison and broad component editing remain open.
