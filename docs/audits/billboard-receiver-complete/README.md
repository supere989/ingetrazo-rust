# Native billboard receiver integration

Production receiver activated through the ordinary Scene / IngeTrazoApp path. One immutable image owner is shared by camera-facing receivers and bitmap shadow casters. Cached placed packets rebuild on scene membership changes; camera updates rotate only sprite vertices. Texture/color runs keep placement order despite shared image storage. Empty billboard parents now retain nested children. Four focused ownership/arithmetic/order/traversal tests pass.

Actual NVIDIA RTX 5090: normal App camera, restore, hide, restore, budget refusal, retry, second resize and close pass. Cold hard-cutout uses one decode/upload/published image, BC7 1024 bytes instead of RGBA 4096; camera and resize do not upload images. Hide retires the image; restore legitimately performs one additional cold upload. Refusal preserves the previous entire owned-window PNG. Existing presentation-fence acknowledgment is used without additional production waits. Final GPU ledger is zero.

Strict 800×600 source-paired captures: 29/36 whole frames exact. Nested-reflection on/restored each retain one shadow-boundary pixel; nested-empty-parent on/restored each retain two. The new equal-depth two-placement mixed texture/color case differs by 1509 pixels in all three phases despite explicit preserved 0/1/2/3 run ordering; precise coplanar arithmetic/depth ownership remains unresolved. This is not an accepted ordering-parity control. Receiver-only off frame of nested-empty-parent is exact. Ordinary solar one-pixel residual remains separately in the prior sample-policy audit. No tolerance changes.

Sampling correction: original pre-super paintGL GL_SAMPLES=0 described widget FBO, not receiver FBO. At actual receiver draw, source FBO2 uses 4 samples. Native receiver captures use 4; packed shadow maps independently remain single-sample. Prior single-sample failures are retained in the earlier sample-policy audit, never relabeled.

Each original bundle preserves IGZ, source pin, exact helper SHA, original GPU identities/matrices and raw frames. Each native bundle preserves binary SHA, full build-source commit and runtime/resource controls. Base/heldout and hard-cutout native frames use 3da2ce34; new ordering/nested and App frames use 42a3be57. The later nested traversal fix does not relabel earlier binaries. Compact captures.zip retains all raw outputs; manifest verifies every byte.

Replay (inside devbox):

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/check-billboard-receiver-evidence.py docs/audits/billboard-receiver-complete
```

Expected exit 1 means strict remaining parity failures, with application/resource controls verified. Missing records, hashes, extents, source pin or resource acceptance cause assertion failure. No full billboard/shadow row closure, physical-Mac proof or selection/Move-preview claim. All owned input-director seats and disposable app/server/GPU processes are stopped.
