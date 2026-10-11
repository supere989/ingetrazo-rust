# Material-rich native glTF/GLB exporter

This batch replaces the positions-only exporter with the operative pinned `formats.gltf` consumer rules: front paint grouped by color/image basename and rounded opacity; double-sided materials; named-material finish or ordered finish inference; PBR roughness/metal/transmission/IOR; original embedded PNG/JPEG bytes; repeat sampler; cutout MASK and opacity BLEND precedence; normals, deduplicated vertices and UVs with glTF V inversion; baked nested Qt placements and geographic extras. Back paint is intentionally ignored by the original exporter and remains ignored here. Source exporter hierarchy is flattened into one mesh/node, not copied as a group node tree.

Nine authored source recipes are saved as IGZ, reopened in the original and Rust, then exported by the real consumer functions. The independently decoded outputs agree exactly on material recipes, names, image MIME/bytes, samplers/extensions, scene metadata, normal/UV tuples, bounds and painted surfaces. Eight have identical oriented triangle collections. The solid planar holed recipe has different internal earcut diagonals: the checker independently proves exact rational coplanarity, oriented boundary and signed area from decoded float32 vertices and identical normals. This is surface equality, not triangle-topology equality; the distinct decompositions remain in the retained GLBs. No pixel tolerance is used. Seven mutated outputs, including a moved boundary and reversed triangle, are rejected by the same validator.

The retained initial affine failures exposed two real implementation errors: float32 mapping instead of the source nested-merge double mapping, and float inverse calculation instead of Qt's double inverse rounded to float. They are rejected by the current validator; no coordinate/color fitting was used. Source pole/slanted normal rounding follows its double Newell accumulation, float qHypot scaling and float division.

Five focused format tests pass, including invalid geometry refusal before any writer bytes are published. No broad suite or Blender render was run. The named executable is frozen and hashed before handoff. One source symlink setup failure was corrected before the successful build; it is not counted as exporter acceptance.

Reproduce only the focused consumer:

```sh
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-gltf-rich.py /tmp/gltf-rich-original > /tmp/gltf-rich-cases.txt
cp /tmp/gltf-rich-cases.txt /tmp/gltf-rich-original/cases.txt
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -p ingetrazo-formats --example gltf_rich -j2
mkdir -p /tmp/gltf-rich-native
while read name; do /tmp/ingetrazo-ai-registry-target/debug/examples/gltf_rich /tmp/gltf-rich-original/$name.igz /tmp/gltf-rich-native/$name.glb; done < /tmp/gltf-rich-cases.txt
python3 scripts/check-gltf-rich.py /tmp/gltf-rich-original /tmp/gltf-rich-native
```

Remaining full-scope requirements: camera-facing billboard figure export/update; broader source triangulation ties/pathologies and textured-hole decomposition; exhaustive arbitrary material-name/parser grammar; source transform degeneracy/weld edge cases; actual Blender image acceptance and native importer extension support. This does not claim a global heap cap or recoverable allocator OOM. Export collects all geometry and retains position/normal/UV/index arrays and final binary simultaneously; existing embedded bytes are borrowed, each encoded asset is read/decoded once per export, and no intermediate full Mesh or ancestor subtree clone is needed. Decimal vertex-key conversion remains cold per-corner work. The writer API prepares before publication but caller-owned file creation/I/O atomic replacement is outside its contract.
