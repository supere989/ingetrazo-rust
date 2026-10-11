# Sparse console integration and Vulkan allocation probe

Integrated runtime c6807387 passes one combined workspace run: 801 tests, zero failures, two existing ignores, 35 test/doc targets, and all enabled examples checked. The changed optional engineering console passes eight merged tests. The combined run took 74.723 seconds; the optional test command took 179.803 seconds including initial dependency compilation into the stable integration target. These are observed batch timings, not a general speedup claim.

Independent replay of frozen console source 9d825a65 verifies 14 immediate original/native states, five changed-row publications, zero final native Mesh exports, seven VM history steps, 17 comparison negatives and nine refusal negatives. The real allocation refusal preserves document, Undo/Redo and revision, retires the VM, leaves older document history usable and permits a fresh successful retry. The prior 11-state history corpus also passes with 11 negatives. See root-review.json for remaining costs and the unchanged default-off boundary.

Published aec2812c completed all six Apple metadata/framework/compute jobs successfully. Linux failed only when the allocation probe assumed zero startup ownership; the new default annotation stencil owns 1,024,000 bytes. The retained preceding-linux-ci.zip records that failure and exact digest. The corrected probe preserves each heap's starting ownership, admits the image before refusing staging, verifies no leaked reservations, resizes the actual application twice and closes with zero committed bytes. This local runtime check used llvmpipe; it is not NVIDIA acceptance.

The frozen original NVIDIA shadow oracle is recorded separately in ../solar-shadow-source. New native Vulkan shadows and Metal full-material acceptance remain isolated and open. No status row or release gate is promoted by this batch.

Raw validation and independent replay files are retained in the ZIPs with entry manifests. acceptance.json contains exact commands and observed timings; preceding-ci.json describes the preceding published commit, not this one.
