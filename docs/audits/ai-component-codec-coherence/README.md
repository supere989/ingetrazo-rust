# Component codec ownership correction

A valid original-saved/loaded identity component was rejected by 2cfe3111: IGZ load puts the prototype token on Group.mesh_share, while its redundant ComponentDefinition snapshot has mesh_share=None. Their mesh payloads are equal. The strict token-equality gate therefore rejected supported identity input, before any affine extension. This real failed execution is retained below.

The corrected admission permits an absent definition token; explicitly linked definition tokens must still agree, definition geometry/empty-child checks remain, and the Group's real mesh-share registry is validated before hydration. Native-created component definition identity remains preserved. Actual original-input IGZ regression covers the loader representation and true Mesh aliases through the app.

Focused consumer tests: 13 PASS, including native component identity plus missing/stale definition refusal. Previous named imported consumer on corrected source: 19 full typed states exact against unchanged pinned source corpus, 11 same-validator mutations rejected. No broad suite. Source/native evidence reuses docs/audits/ai-console-imported-mesh; hashes below identify actual corrected execution without duplicating unchanged captures.

Retained failure: first new affine fixture (identity seed only) returned Engineering Python refused: Imported component definition is not coherent on initial a,b=groups inspection. Diagnostic showed identical Mesh payloads, Group.mesh_share=Some(proto token), ComponentDefinition.mesh_share=None. No affine proof is inferred from this correction.
