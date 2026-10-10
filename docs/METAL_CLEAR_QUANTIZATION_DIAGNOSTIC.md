# Native clear quantization diagnostic

The actual 287d9b2 Intel presentation run met the unchanged positive-display and resize gate (9 displayed, 13 retired, no pending owners). Its owned-window PNG nevertheless fails the exact screenshot gate: red is 21 rather than 20. ARM captures red 20 but still fails positive presentation callbacks. These are separate verdicts.

Retained complete native artifacts and bounded ICC inspection are in `docs/audits/metal287-native-color-review`. Both PNGs carry ICC profiles, without sRGB/gAMA chunks; ARM describes Apple Virtual and Intel Unknown Display. This difference is evidence, not a proven cause. No expected pixels or tolerances changed.

Build `cargo build --locked -p ingetrazo-metal --example clear_quantization_probe` on macOS. Run `clear_quantization_probe --output=FRESH_DIRECTORY`, then `python3 scripts/check-metal-clear-quantization.py FRESH_DIRECTORY`.

The diagnostic renders the same fractional clear into a native private BGRA8Unorm target and performs GPU-to-host readback through existing bounded completion ownership. It records fractional conversion without inventing an expected byte; exact byte clear, black, white, uniform whole-image, invalid-clear admission and zero final ledger are mandatory. This is offscreen target readback, not CAMetalLayer drawable readback and not proof of WindowServer color processing. Production host and presentation acceptance are unchanged. Both Mac target crosschecks passed; actual native execution remains required.
