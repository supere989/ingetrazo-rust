# Authoritative style and shadow recipes

Source `1ebd8352`, original Python pinned `6be29fe437a7cd992d4e079f71720821b25216b9`. Fresh operative original StylesPanel/ShadowsPanel/ScenesPanel and History controls match the production Rust app controller, native IGZ save, and saved-view recipes. Four context cases, 16 states compare exactly; stale style, stale shadow clock, wrong Undo/Redo depths and empty corpus are rejected. Earlier literal app baseline remains in `../document-presentation-baseline` at `36502a42`.

Document RGB/time values stay f64 in storage. Renderer-facing f32 colors are compared by explicit float32 conversion, without tolerance. Presentation changes remain outside model History and survive ordinary/folded and nested Finish/Undo/Redo; presentation-only Finish does not refit unchanged geometry. Full workspace: 544 passed, two ignored.

Replay `check-document-presentation.py original.json native.json` and `check-presentation-contexts.py contexts-original.json contexts-native.json`. Actual GPU styles, rendered shadows, image planes, preset library, richer units, tile state and full settings parity remain open. This proof does not cover rendered panels or native Metal runtime.
