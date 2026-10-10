# Independent acceptance protocol

The pinned Python-generated example files are the reference corpus. Run from the repository root after recursive submodule initialization:

```sh
cargo test --workspace --locked
cargo build --locked -p ingetrazo-cli
python3 scripts/check-document-parity.py
```

The document checker fails closed if any of the four named fixtures is absent. It recursively checks every original scene field, including component prototypes, group transforms, geometry coordinates, attributes, saved cameras and sheet metadata. Added defaults are tolerated; original list order and schema must survive. Numeric comparisons allow only 1e-10 relative/absolute rounding. Every non-document ZIP member must preserve its SHA-256 content hash. Unknown metadata is part of preservation acceptance. Five in-memory negative controls deliberately remove metadata/groups or corrupt geometry/transform/material; all must be detected. Comparison errors are retained as JSON paths.

This is untouched-document preservation, not a claim that preserved unknown data can be edited. An independent edited-save acceptance must modify modeled geometry/transforms, save, and establish both the edits and retained metadata. Python reopen, recursive instance expansion, texture rendering and GUI user workflows remain separate acceptance gates. The checker reads Python-format source documents independently of Rust serde but does not run the Python application.

`boolean_acceptance.rs` computes signed tetrahedron volume from boundary coordinates without calling the implementation volume/manifold methods. For overlapping unit boxes analytic union/difference/intersection volumes are 1.5/0.5/0.5. Oriented edge incidence independently checks closed output. Copy-A and concatenate-boundaries negative controls prove old placeholder geometry fails this oracle. This bounded corpus does not cover arbitrary curved, degenerate, rotated, material or undo cases.

CI distinguishes Rust tests/build from a separately named semantic document step; retained evidence uploads even on semantic failure. Baseline files retain observed losses and fake booleans before implementation repair. Red semantic acceptance is intentional while blockers remain; do not replace it with counts or silently skip examples.

## Original runtime reopen

`INGETRAZO_BINARY=target/debug/ingetrazo python scripts/check-python-reopen.py` runs the pinned original loader and serializer on both the source and Rust-roundtripped file. Install original runtime dependencies in an isolated environment; the retained Python freeze records the local run. Headless Qt uses offscreen mode and a temporary cache. Older sources lack group names and persistent UIDs, so the checker freezes only the original runtime generators at the start of each load. Existing stored names/UIDs remain authoritative. This removes nondeterministic original behavior without discarding identity fields from comparison. Original runtime scene geometry, annotations, composer and other serialized document data are compared recursively. This still does not establish rendered or interactive acceptance.

`check-python-edits.py` creates source documents, edits them in Rust, and verifies actual original-runtime geometry, persistent IDs and transforms. It also reopens fresh Rust saved views, section planes, geographic datum/points/paths and sheet layouts. Unknown source metadata is checked in saved JSON, since the original application itself omits opaque fields it cannot interpret.

`check-python-booleans.py` compares union, subtraction and intersection on overlapping, rotated and contained boxes (nine cases). Rust outputs are reopened by the pinned Python application. Both results pass through the original native manifold3d kernel; volumes and bidirectional difference volumes must agree within 1e-6 cubic metres. The rotated case differs by less than 1.4e-8 cubic metres due to the original QVector3D float precision. A copy-input negative control must disagree. This checks spatial solid equivalence rather than polygon counts or the Rust volume method. Rust uses manifold-rust; the independent original oracle uses native manifold3d. Degenerate/general geometry, attributes, picking and undo remain outside this bounded shape test.

`check-python-pushpull.py` compares the original tool classify/commit path against Rust outputs for cap extension, inward cap movement, blind pocket and through-cut. All four independently reopened solids have zero bidirectional native manifold difference. Original subtractive/orientation regression modules separately pass 42 tests; these establish the original oracle works, not that all 42 have Rust counterparts. The independent Rust cases also check hole rings/inner walls, nonfinite and invalid-face atomicity, and unrelated solid/wire preservation. Full original prism/recess/through-notch/slanted/instance corpus and interactive input remain open.

`check-python-followme.py` compares four connected sweep constructions (straight, 90 degree bend, holed tube and closed rectangular torus) against the original engine with zero bidirectional native manifold difference. A straight-result negative control disagrees with the bent shape. Original Follow Me/revolution reference modules separately pass 24 tests. Three independent Rust tests additionally cover branch rejection, malformed path atomicity and bent-path material propagation. Axis-touching/crossing revolution poles and full interactive selection remain open.

`check-python-components.py` verifies real Python object identity after Rust shared-definition serialization: linked instances reopen with the same mesh, plain groups remain independent, a registry edit updates both linked instances, Make Unique separates future updates, and persisted instance IDs and opaque attributes survive. This bounded definition probe does not cover recursive editor/clipboard/undo workflows.
