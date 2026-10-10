# Second reviewed swarm milestone

Implementation base: `2da7c62`. Original submodule remains pinned at `6be29fe437a7cd992d4e079f71720821b25216b9`.

- 170 Rust tests pass ([log](swarm-milestone-02-tests.txt)).
- Explicit shared component registry, nested local edit transactions and single outer undo entry; Make Unique, linked copy/paste and original runtime reopening retain intended identity and placement transforms. [Independent evidence](python-component-evidence.json).
- Group/explode preserves holes, face/edge attributes, valid topology and promoted nested transforms. Save uses temporary same-directory output and atomic rename; embedded texture basename collisions are disambiguated.
- FollowMe straight, miter bend, holed tube and circular torus match original native shapes exactly. [Evidence](python-followme-evidence.json).
- Fillet creates real single-edge rounded geometry. Three radii/segment configurations match original native shapes within 1e-6 tolerance, maximum difference approximately 4.67e-9. Independent unchanged-geometry controls reject the old soft-edge placeholder. [Evidence](python-fillet-evidence.json).
- Invalid tool preparation reports a visible error without scene/history mutation.

These are bounded fixtures and controller transactions. Arbitrary sweeps, pole clipping, multi-edge/spherical Fillet joins, loose-geometry clipboard and rendered gestures remain open. No full tracker row is Verified.

First milestone cloud acceptance passed: https://github.com/supere989/ingetrazo-rust/actions/runs/38010979180. Second milestone cloud result is reported separately after publication.
