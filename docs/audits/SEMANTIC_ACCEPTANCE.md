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
