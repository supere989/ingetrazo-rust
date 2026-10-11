# Raw-loop OBJ reference fusion

Reference branches now borrow parsed point/attribute slices and build source round-four keys, signatures and normals once, without first constructing native Mesh vertices/edges/IDs. Regions trace their first authored coordinates; only emitted output enters the tolerant Mesh registry. Editable/native-double and legacy paths are unchanged. Native output faces use original _add_fused fallback: reject a fused candidate, retry each region original with region paint; a rejected original does not discard its siblings. Singleton rejection also retries its original.

Three new actual source workflows/15states cover cross-cell near welding, same-cell far welding, and textured duplicate winning coordinates. Source admission fallback has three explicitly fault-injected helper controls, with attempts, full loops and opaque paint compared by a focused native emitter test; no claim that default source naturally rejects those shapes. The old frozen binary independently differs in six import/Redo/final states, proving real cross-cell/duplicate defects; same-cell remains a positive contract control. No source oracle normalization or existing winding whitelist change.

New mode: context_document_lifecycle DIR --obj-raw-fusion; original reader scripts/reference-obj-raw-fusion-reopens.py DIR; strict checker scripts/check-obj-raw-fusion.py DIR. Existing fused12/MTL/nested sources are reused unchanged. Source/proof records retained until root release. No rendered/all-library/P22 promotion or total heap bound.

## Final runtime acceptance

Frozen runtime `736ab5c0`, checker `e320a894`: three actual workflows / 15 full states and three actual pinned reader reopens pass, seven mutations plus the retained preliminary-Mesh candidate reject. The one source-derived emitter test passes all three explicitly injected admission failures. Binary SHA256 `7c9d163897fbd852b57bea6b4aa7d058fa8775d1a93bf1e185464330321a72dd`. Build 17.63s; focused test build 5.78s.

Final-binary affected replay passes 55 deterministic fusion states, 75 MTL states and 105 nested states (plus their retained controls/readers). The plain fusion checker remains nonzero for exactly the retained 16 winding leaves; its whitelist is unchanged. `records.zip` authenticates native outputs, initial failures, logs and affected JSON receipts. Edge-flag inventories compare numeric endpoints because Python and serde scientific notation differ lexically; no coordinate, ring, flag or owner field is changed. Raw source records remain unchanged. Full P22 remains open beyond this subset.

## Trace-refusal correction

`b778c183` distinguishes source fusion trace refusal (each original keeps its own attrs and singleton retry) from emitted fused-face admission refusal (originals inherit region attrs). One natural three-triangle non-manifold shared-edge witness has compatible rounded UV signatures but three distinct authored fits. Actual source and native emitter records match; both focused fallback tests pass (3.04s build). No old App corpus recaptured/replayed. Root joint build owns final consumer validation of this followup.

`append_raw` validates its inputs before inserting vertices. It passes only IDs returned by that same Mesh to `add_face`; the latter refuses invalid IDs before edge publication, so its None path is unreachable for these validated IDs. Fault admission occurs before append. This does not assert parity for unobserved source constructor exceptions.
