Actual Qt6.12 mandatory200 scale cases are exact, including4x4 identity and4xN/Nx4. Prior compiled kernel retains12 failures. Non-x86 reciprocal dispatch remains subject to actual ARM Qt replay.

Actual Capilla JPEG decoding remains a strict failure for all four assets. Three smaller scaled outputs happen to match;1392x1144 differs in three channels. Feeding actual QImage RGBA to the native scale kernel reproduces all16 pixels exactly for every asset. No production average caller is activated.

Replay native_texture_average_assets with the retained capilla IGZ and fresh raw directory, then scripts/check-skp-texture-average-assets.py IGZ native.json rawdir qt_texture_average_protocol result.json. It deliberately exits1 for decoder differences. Source inputs reside in ../skp-real-model-nvidia-failure/original. Raw diagnostics may be purged after retained results.
