# Clipboard Mesh prototype identities

Clipboard resources carry `Scene.mesh_shares`, validate prototype references,
and merge explicit tokens with conflict refusal. Paste and preview no longer
replace every component definition: copied placement trees retain their own
child lists. Same-document copies refresh cached Mesh data from current tokens
while preserving each copied placement matrix and child list.

Foreign import uses a runtime cache keyed by destination document, source
document, source token and remapped prototype signature. A cached target is
reused only if its registered prototype still matches; otherwise an unoccupied
fresh UUID is allocated. An occupied equal prototype is never treated as an
identity match unless that import cache establishes the source identity.
The cache retains only signatures referenced by the current payload, bounding
its size by the current prototype count rather than imported history. Changed
signatures evict old mappings; repeated current signatures stay stable. It clears on new local
Copy, and is outside Scene/history/native serialization. Hash collisions cannot
silently overwrite geometry: target prototype equality is verified before reuse.
Changed source snapshots get separate identities from already-pasted geometry.

Resource-name remapping covers prototype face paint/texture references, including
embedded images, while preserving UVW and opaque fields. Two independently
authored equal source prototypes stay separate; repeated paste of one source
prototype links the new placements to the existing imported token. Definition
root and child token references are remapped together.

Fresh clipboard tests: 12 PASS, 1 isolated-system-clipboard test ignored.
New controls cover independent parent child lists, live same-document prototype
refresh, foreign token collision with equal destination geometry, equal-but-
independent source prototypes, repeated import, image-name collision, opaque
texture metadata, modified snapshot separation and exact Undo/Redo registries.
These are native controller/resource proofs. Cross-process original clipboard
and rendered paste visibility remain separate acceptance scopes.
