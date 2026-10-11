# Actual NVIDIA solar extra controls

The archive preserves five cases at frozen source 1fa5d33e and one corrected bitmap replay at 2fb02bd7. Exact full source commits, executable SHA256, actual adapter, source JSON digest, transport command and completed results are retained inside each directory. Reference: root oracle /tmp/ingetrazo-solar-source-extra-3, pinned original 6be29fe437a7cd992d4e079f71720821b25216b9. Original images are not duplicated here. All comparisons use unchanged exact RGBA criteria.

At 1fa5d33e bitmap extraction missed the embedded identity and produced an empty map. This failure is retained. At 2fb02bd7 bitmap packed map is exact at all 2048x2048 pixels, with 397100 occupied pixels in both. On/off/restored each differ at 11678 receiver pixels: source camera-facing billboard receiver rendering is not closed by this caster fix. Mesh face-me packed map remains empty in both (not a positive visible shadow control).

All six executions passed three unchanged warm updates, uniform allocation refusal preserving the previous full frame and ledger, off/on restoration, and final ledger zero. They use actual RTX5090 native Vulkan and source GL single-sample target policy; production multisample and WSI/app dispatch are not proved by this headless slice.

Terrain/photo oracle controls inject renderer readiness which their IGZ does not serialize. Native terrain/base-map guard is derived from typed Scene ownership; photo readiness has no current typed native API and remains OPEN. Their pixels must not be presented as a completed suppression proof. Tiles packed map is exact; on/off each retain 558 frame differences.

Replay transport is retained verbatim. It requires rootless Podman merged filesystem access (`podman unshare /usr/bin/python3 transport.py`), the publish container, readonly NVIDIA CDI runtime and an existing named probe binary; SOLAR_SOURCE/SOLAR_OUT/SOLAR_CASE select the fixed corpus/output/case. Rebuild frozen source with reduced-debug target and INGETRAZO_BUILD_SOURCE equal to full commit first. Verify the executable SHA against the execution record; do not relabel a later binary.
