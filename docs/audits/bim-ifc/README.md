# Rust BIM tagging, takeoff and IFC export

The actual Rust application now groups selected loose faces under one new BIM id, tags groups with source name fallback, supports Undo/Redo and whole-object selection, shows per-class metrado and closed-shell volume, and writes quantities CSV through the save dialog. The quantity cache is keyed by model revision, Scene version and editing context: 100 unchanged refreshes produce one measurement. IFC export includes tagged objects only and uses the same world geometry for shape and measurements. Open face sets use surface models; edge-closed sets use faceted BReps. Polygon holes are preserved. Group tags survive native IGZ save/reopen.

Twenty pinned original cases cover all 15 curated classes, an open wall with a hole, a rotated door, a transformed hidden parent with nested child, unknown-class fallback and untagged refusal. Nineteen actual IFC files pass independent IfcOpenShell 0.9.0 reading and full IFC4 EXPRESS validation, with exact source quantities, world polygon/hole coordinates, element classes/names, Door/Window dimensions and open/closed representation checks. The twentieth case refuses untagged export. Nine checker mutations are rejected. The independently found initial IfcSpace containment/decomposition failure is retained: native spaces now aggregate under their storey.

Application tests cover grouping, selection, cache reuse, real CSV/IFC files, IGZ tags, Undo/Redo, empty selection and exhausted ids retaining older Redo. Export refusal controls cover nonfinite coordinates, arithmetic overflow and six-decimal corner collapse; each preserves an existing destination and removes its staging file. Publication stages beside the destination, syncs, then renames. A failure syncing the parent directory after rename explicitly reports that replacement already occurred.

Important scope boundaries:

- The original collect_objects uses world-space group area/volume, but its CSV/IFC class quantities and IFC geometry use local group.mesh. Native consistently measures/exports transformed nested world geometry. This is a documented correction, not a claim of byte-for-byte compatibility with that source defect.
- Six-decimal IFC serialization matches the source convention. Arbitrary coordinate/quantity precision remains unverified; collapsed ring corners are refused.
- Closure follows the source's two-incidents-per-edge criterion. It does not prove consistent orientation, self-intersection freedom, or general solid validity.
- Tag-as-you-draw requires authored draw-plan identity and attribute propagation. Its existing inert checkbox is disabled with an explanation; that feature remains OPEN. Full document takeoff/CSV while group editing is active is refused until that workflow is implemented. Actual UI gesture and broader component-registry acceptance remain OPEN.
- World geometry is prepared once per object; one measurement provides area, volume, class quantities and IFC shape classification. Table refreshes reuse their cached result. This is a cold quantity path, not a representative large-model performance claim.

Reproduce after extracting reference.zip into an output directory:

```sh
cargo run -p ingetrazo-gui --example bim_ifc_fixture -- /path/original /path/native
python -m pip install ifcopenshell==0.9.0 pytest==9.1.1
python scripts/check-bim-ifc.py /path/original /path/native --output result.json
```

Python is used only for the original oracle and independent validator. Rust runtime tagging, measurement and export have no Python/IfcOpenShell dependency.
