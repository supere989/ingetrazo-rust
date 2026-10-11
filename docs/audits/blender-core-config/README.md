# Blender core configuration acceptance

The real pinned original `write_job` and native `build_job` agree exactly on 21 controlled job configurations. The original actually exports its GLB while constructing each job; Blender execution is outside this proof. Only temporary GLB/output paths are normalized. Three environment and four Linux/Flatpak cache configurations also match exactly. The same strict validator rejects five altered results.

Camera controls cover poles, roll, fractional float targets, near eye, two point and large float targets. Native admission refuses nonfinite and non-QVector3D-representable target/up values, and derived float overflow; callers must invoke `validate_camera` before cold and warm submission. Ground controls exercise orphans, hidden layers/faces/root and hidden/billboard intermediate groups with visible grandchildren. Solar controls include site north, night and invalid calendar fallback. The night-light control exercises Kelvin precedence and clamps.

This is core configuration evidence, not rendered Blender pixels, process ownership or full saved-light/material parity. Native macOS/Windows cache locations remain unverified. Arbitrary native double placement/camera inputs outside Qt's representable source domain are not claimed as compatible. Runtime environment cleanup uses the non-PyInstaller path; the explicit frozen helper is independently tested.

Reproduce in the isolated publish container:

```sh
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-blender-core-config.py > /tmp/blender-core-original.json
CARGO_TARGET_DIR=/tmp/ingetrazo-blender-core-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo run -p ingetrazo-core --example blender_core_config -j2 > /tmp/blender-core-native.json
python3 scripts/check-blender-core-config.py /tmp/blender-core-original.json /tmp/blender-core-native.json
```

No broad suite was run. Source and executable digests are in `provenance.json`.
