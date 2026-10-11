# Native appearance preferences

Runtime `792010fb` implements source theme and toolbar-size choices through actual native Preferences/App consumers, including composer controls. Frozen source 11 states and all 110 source QPainter icon assets remain unchanged. Literal native palette/settings states and native decoded RGBA plus embedded PNG hashes match exactly. Six state mutations and 110 one-byte ink mutations reject. Extra native guards cover Unknown immediately after Light→Dark fallback, preserved document/history and opaque preferences, invalid raw value across restart and unchanged warm cache.

Actual egui owner test passes 1 (345 filtered): real Theme and size combos, unpublished draft, byte-preserved Cancel, OK, restart, composer Text control heights 44/24 for 40/20 px, and stable warm cached TextureId/count. Existing compositor ingress locators use composer-add-text rectangles, which are preserved. Native compositor only forwards shared size to its existing toolbar/control geometry and renders the existing Text action with the real source Text icon; it does not claim full original toolbar layout or all compositor tools.

The original proof uses the unchanged pinned PreferencesDialog and literal MainWindow toolbar callback on a real QMainWindow/QToolBar, actual Qt palettes and simulated OS scheme events through the source follower. Native OS scheme observations use actual egui RawInput. This is bounded controller/UI/asset proof, not desktop OS event or whole rendered application pixel parity. Theme palette setup is cold per context; subsequent frames compare small preferences. Icons decode/upload once per `(name, dark)`, retain both palettes, and changing size reuses the same 48 px source asset. No per-frame Scene clone, file read or persistence is added.

Named build 17.85s after local-worktree handoff invalidated only nine local-path packages, retaining external dependencies. Exact UI test compile 5.10s/run 14.52s. No source recapture, broad suite, GPU replay or unrelated compositor check. No runtime candidate failure/retry occurred.

Canonical manifest authenticates every raw archive member, runtime/original source and exact tested binary. Raw proof remains /tmp/ingetrazo-appearance-proof until root release; reusable target/worktree are retained.

Replay using the existing named context_document_lifecycle binary:

```sh
context_document_lifecycle FRESH_REPLAY_DIR --appearance
python3 scripts/check-appearance-preferences.py FRESH_REPLAY_DIR
```

Extract original-appearance.json and source-icons.json as the only required frozen inputs; source PNG images remain archived for audit. The binary emits native decoded assets and native JSON/control files. Owner test is appearance_preferences::tests::actual_appearance_choices_cancel_restart_and_composer_cache. Wider P33 coordinate entry and remaining preferences consumers remain open.

Root integrated the exact tested runtime as `61e2b978`. Complete `crates` tree, Cargo manifests/lock and build configuration paths are identical to owner `792010fb`; integration-review.json pins that comparison. Root therefore reused the independently authenticated owner binary/UI proof without another build or replay. Runner `41fcf1e4` adds only this mode's two frozen inputs and producer-dependent checker to the existing combined roster. The next stable combined publication receives one broad CI run.
