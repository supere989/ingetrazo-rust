# Reflected glTF source expectation

The Linux CI job at `a9cd0eea`, run 38118894709 / job 114409352452, failed the old export checker expectation of +Y normals and +Z winding. The source-matching exporter changed in `85bfe6ad` (the rich export batch); the old checker still assumed an independent outward-normal repair.

A fresh invocation of the original exporter pinned at `6be29fe437a7cd992d4e079f71720821b25216b9` with exactly the CI fixture's two transforms produces **-Y encoded normals and -Z decoded winding**. The source preserves reflected corner order. `reference-export-reflection.py` reproduces the retained original GLB and independently decoded JSON. The checker now requires that exact orientation; its coordinate, buffer, layout, length and index checks remain intact. This changes the stale acceptance expectation, not runtime behavior or tolerance. The original failure remains recorded here.
