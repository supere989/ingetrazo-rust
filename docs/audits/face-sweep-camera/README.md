# FaceSweep native camera/depth correction

Runtime `068fa107` closes both retained reversed-door full-frame failures on the actual RTX 5090/NVIDIA615.71.09. All six actual application source-frame consumers now match all 24 complete 800×600 RGBA reference frames exactly, including source length labels. Source originals were reused byte-for-byte; none were recaptured. The strict checker rejects transparent blank, opaque white blank and one-byte mutation for every frame (72 negatives).

The causal diagnostic, built on base7b3afb0e, held the scene, swept geometry, material, draw order and MSAA4 constant. Ordinary camera/depth mapping reproduces preview539/max81 and offset1079/max122. Existing `set_scene_with_sun(None)` routes the same scene through source Qt-f32 VP plus negotiated native GL negative-one-to-one depth mapping: both become0/0. The forced portable source-depth fallback uses the same Qt VP but pre-rounds the homogeneous Z row and reproduces539/1079. This narrows the mechanism to native GL depth mapping, rather than motion distance or geometry. The portable path remains a strict precision gap; no offsets or tolerances were introduced.

Original Viewport projection/view QMatrix4x4 values and GL viewport-depth law are already retained in face-sweep-label-source. Native reuses the reviewed navigation_ray source CameraSnapshot and source_depth negotiated dynamic state. The new helper factors the existing solar camera policy without changing it. Active FaceSweep routes through this helper in both production host and headless scene transaction. Ordinary non-preview camera arithmetic remains unchanged. Camera admission runs before scene mutation; uniforms publish only after successful scene preparation. There is no Scene clone or additional geometry upload for camera selection.

Motion note: the axis-aligned frozen normal and distances narrow to identical f32 sweep offsets even though original QVector3D dot and native f64 on_move can report different raw lengths. That separate handler gap is still open and was not used to explain these pixels.

Source-frame observer phases include acquired zero-offset base/cancel, not true tool Cancel; the same native run independently executes actual Cancel and Save afterward. This is real App/Tool method dispatch and native World/label presentation, not hardware pointer proof. Prior full transaction/refusal/Undo evidence is retained in face-sweep-vulkan-followup and not claimed as freshly repeated here. Full general-renderer, portable-depth and Metal acceptance remain separate.

Replay retained results:

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/check-face-sweep-camera.py docs/audits/face-sweep-camera docs/audits/face-sweep-label-source
```

records.zip retains all six actual app transports, complete frames, state/cleanup reports and the ordinary/native-GL/portable diagnostic controls. provenance.json authenticates production source, both executable hashes and exact historical diagnostic source. diagnostic-base7b.rs must be built against the stated historical base to reproduce that first causal experiment. Its initial host transport failed to read the Podman merged executable for hashing after the ordinary GPU process completed; the native result and frames remain retained, and subsequent transports hash through podman exec. No failed GPU attempt or retry is concealed.
