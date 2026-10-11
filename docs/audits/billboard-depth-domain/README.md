# Source depth-domain receiver correction

Frozen source d9d20726cbb26d8bb9a59af0ab6f2917febabdd3. Actual NVIDIA RTX 5090, source Qt 6.12.0 / pinned application 6be29fe437a7cd992d4e079f71720821b25216b9. No tolerance changes.

14 changed-path cases: 53 of 56 full images/maps exact. Equal-depth placements, nested reflection, empty parent, ordinary solid and section are now exact in all phases and entire 2048-square maps. Cutout retains on/off/restored failures 441/306/441 pixels (max106/30/106); full renderer remains Partial. Original source references are retained captures, not newly rerendered. Solar source receiver sample4 is the pinned scene FBO policy corroborated by actual billboard receiver hooks; old pre-paint samples0 metadata remains unchanged.

Normal actual application passes camera, hide/restore, refusal/retry, two resized extents, warm shared cache and BC7 controls with final ledger zero. Historical 1509/1/2-pixel failures remain in billboard-receiver-complete/captures.zip; section old packed38486-pixel failure remains solar-application-native/controls.zip. No physical Metal/general renderer claim.

Correction: disable billboard polygon offset as original does; use enabled native negative-one-to-one GL depth control for literal source Qt MVP, including source packed caster projection; use source pre-raster section clipping on surfaces and ground. Unsupported depth-control devices retain checked portable matrix conversion, whose exact pixel parity remains unverified. No additional pass/copy or per-frame scene upload.

Run: /tmp/ingetrazo-parity-python/bin/python scripts/check-billboard-depth-evidence.py. Exit1 intentionally retains the three strict cutout failures.
