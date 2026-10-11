# COLLADA material resolution contract

Worktree `swarm/dae-material-resolution` starts from published `3195c7b6`. No runtime modification or acceptance claim yet. P23 retains its complete existing criterion and all hierarchy/FACEME gaps.

Pinned original `formats/dae.py` defines the missing material behavior directly:

- An unresolved material/effect/sampler binding contributes no paint and does not abort imported geometry.
- Shader lookup has fixed lambert/phong/blinn/constant priority; a usable literal diffuse color precedes texture within a diffuse element.
- Empty image references contribute no color or texture. A missing or undecodable nonempty image receives the original path-derived pale tint; a present undecodable file still supplies a texture path if authored UVs fit.
- Image references use local `urllib.parse.unquote` path resolution without network IO. Missing files must not cause the current whole-import refusal. Preserve forgiving percent decoding, literal malformed percent characters, Unicode names and per-document resource ownership.
- Cache resolution/bytes/representative color per distinct resolved image for the complete import, including shared-prototype rebuilds. Move the prepared asset table into the result once; avoid the current whole-table clone during each prototype decode.

The fallback tint uses Python string hash, so it is stable within a process but varies across randomly seeded original processes. The pinned Python is 3.13.13, 64-bit SipHash13, cutoff0. Honor explicit `PYTHONHASHSEED` for reproducible comparisons; use process-local random keys otherwise. Source Unicode hashes compact 1/2/4-byte code units. `siphasher`1.0.4 is already in Cargo.lock; an explicit formats dependency would avoid an algorithm copied into product code. Verify ASCII/Latin1/BMP/astral vectors and seed0/nonzero/boundary once against the actual original interpreter before claiming parity.

Primary algorithm references: [CPython hash seed expansion](https://github.com/python/cpython/blob/3.13/Python/bootstrap_hash.c), [CPython SipHash selection](https://github.com/python/cpython/blob/3.13/Python/pyhash.c). Seed0 clears the secret; a nonzero uint32 seed expands bytes with the original LCG. This is a format fallback-color requirement, not a Python runtime dependency.

Next implementation belongs in `crates/ingetrazo-formats/src/dae_import.rs` and a small private color helper if needed. Reuse the existing `dae_import_acceptance_fixture`; capture only missing material/image authority fixtures, then one coherent named build and original-reader comparison. Retain existing twelve small/seven reference/four UV corpora without recapture. Existing private helper refusal tests need to distinguish local read failure from whole-document material fallback; do not weaken geometry/index/cycle validation. The isolated worktree has no reserved target yet; both active targets belong to the Freehand and OBJ/MTL owners.
