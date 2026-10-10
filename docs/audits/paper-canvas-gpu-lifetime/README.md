# Actual Vulkan managed raw lifetime admission

Source chain `a52833e3` → `8726b2c6`. Final executable/source hashes and exact invocation: `final-evidence.json`. `hashset-run.log` exits 0 on actual llvmpipe Vulkan surface, presents real egui mesh frame 3, and cleans all GPU ledger charges to zero. No CPU stand-in uploader.

The normal System draw path and probe share `upload_pending_textures`. Existing ordinary managed conversion is unchanged. The probe admits a real 256-byte image, denies its 16-byte staging allocation, and checks rollback, retained managed texture IDs, unchanged submitted-frame serial, raw-byte retry retention, drop cancellation, held retry/ACK, and exact free accounting. A real egui same-frame set/mesh-use/free is submitted and presented before its acknowledged next-frame retirement. Ordinary partial update/free is exercised through the actual renderer as well. This is resource/presentation proof, not an independent screenshot comparison of a previous frame.

## Mutation controls and retained failure

`mutations.json` pins two actual compiled executables from `a52833e3`. Omitting dead-ID cancellation creates an actual orphan managed upload and the probe rejects it. Blanket cancellation discards a texture used by the current mesh, causing the actual render path to reject `Bad texture ID: Managed(3)`. Both exit 101 and clean GPU charges to zero. `a528-mutator.py` is the exact historical mutation recipe. The runnable script is updated for the equivalent linear HashSet implementation, and fails if its source needles are absent. The subsequent HashSet-only source change is independently exercised by the final positive GPU run; historical mutation evidence is not relabeled as that newer source.

`initial-run.log` retains an initial fixture failure: adding a quad only in the test frame grew geometry buffers by 280 bytes, invalidating a whole-ledger baseline comparison. The final fixture warms identical quad topology before baseline capture; thresholds are unchanged. `final-run.log`, `paint-aware-run.log`, and `restored-run.log` retain earlier positive stages rather than replacing their provenance.

## Scope still open

Early no-orphan cancellation is proved for meshes without opaque callbacks. If any callback exists, the host conservatively retains current-frame IDs and uses acknowledged next-frame post-fence retirement; this does not establish early no-orphan behavior for composer CAD callback frames. No callback identity guessing, no driver/hardware claims beyond the recorded actual adapter. Native Metal/Glow capability and source canvas fidelity remain separately owned stages. Outer-border and general clip/font/layout fidelity remain open. These checks do not promote full P29 parity.
