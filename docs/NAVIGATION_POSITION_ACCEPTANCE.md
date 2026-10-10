# Position camera independent acceptance

The frozen original commit is `6be29fe437a7cd992d4e079f71720821b25216b9`, using Python 3.13 / Qt 6.12.0. Original handlers run in the real MainWindow; no replacement status bar, placement algorithm, or tool handoff. The controlled native controller uses production navigation input. Sixteen cases compare literal float cursor points, snap kinds, handoff, immutable model history, and previews; existing camera placement uses the pre-existing 2e-5 Qt numeric boundary. Seventy-two separate original viewport rays compare origins, directions and plane hits with literal equality, including perspective/two-point/translated targets and standard/oblique views. Missing, altered-point/ray, absent-preview, premature-history and wrong-handoff negatives must fail.

Position's dedicated unprojection implements Qt 6.12 float construction/multiplication/map, double-cofactor inverse rounded to float, and QVector3D plane arithmetic. It does not alter ordinary camera/render/pick rays. Source references: [QMatrix4x4](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/math3d/qmatrix4x4.cpp), [inline map/multiply](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/math3d/qmatrix4x4.h), [QVector3D](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/math3d/qvectornd.h). No position rounding table or widened tolerance.

First-point bounds follow the original visibility policy: visible loose edge endpoints and face outer loops, full meshes of visible non-billboard placements, no loose orphan vertices. The original includes draw-hidden edges in bounds and visits visible descendants of hidden nested placements; the literal controls preserve that behavior. Position caches center/index by scene/view/model revision. Existing generic first-point callers receive the corrected bounds policy too.

The compiled earlier `11250fab` controller is independently replayed using `scripts/position_first_point_old_defects.rs` copied as an example in a separate checkout. It shows six noncommitting front/image/model-center placements and an absent drag preview. That fixture adapts only the old input return type and absent observational preview API. Source and binary SHA256 are retained. When sequentially reusing a terminal target under disk pressure, clean the five workspace package outputs before restoration; retain dependency caches and never run compilers concurrently. A stale-render-rlib failure is retained.

To replay controlled comparisons from a fresh checkout with its own target:

```sh
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 /path/to/pinned/python scripts/probe-python-position-first-point.py /tmp/position-source.json
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 /path/to/pinned/python scripts/probe-python-position-rays.py /tmp/position-rays-source.json
cargo run --locked -p ingetrazo-gui --example position_first_point_acceptance > /tmp/position-native.json
cargo run --locked -p ingetrazo-gui --example position_ray_protocol -- /tmp/position-rays-source.json > /tmp/position-rays-native.json
python3 scripts/check-position-navigation.py /tmp/position-source.json /tmp/position-native.json /tmp/position-rays-source.json /tmp/position-rays-native.json /tmp/position-result.json
```

Rendered acceptance uses production native Vulkan or real original Qt/OpenGL on owned Xvfb/llvmpipe windows. The runner sends actual XTEST pointer/button/Escape events, observes camera/model/preview state, and captures only that application window. It requires a presented nonblack viewport, a visible Endpoint glyph, and at least 95% preview coverage along 48 projected interior samples. Baseline missing-preview/glyph and black-world controls must fail. Press/drag do not mutate camera/model/history; release hands off to Look; Escape removes preview and preserves camera/history. It does not establish whole-image glyph/AA equality or hardware performance. The original window supplies its own native winId; native window discovery requires one IngeTrazo window in the harness-owned display.

```sh
cargo build --locked -p ingetrazo-gui --example position_navigation_native_fixture
CARGO_TARGET_DIR=/path/to/target /path/to/pinned/python scripts/check-rendered-position-navigation.py /tmp/native-position-new
INGETRAZO_POSITION_CANCEL=1 CARGO_TARGET_DIR=/path/to/target /path/to/pinned/python scripts/check-rendered-position-navigation.py /tmp/native-position-cancel-new
INGETRAZO_POSITION_ORIGINAL=1 CARGO_TARGET_DIR=/path/to/target /path/to/pinned/python scripts/check-rendered-position-navigation.py /tmp/source-position-new
INGETRAZO_POSITION_ORIGINAL=1 INGETRAZO_POSITION_CANCEL=1 CARGO_TARGET_DIR=/path/to/target /path/to/pinned/python scripts/check-rendered-position-navigation.py /tmp/source-position-cancel-new
```

Full navigation, cursor capture/platform support, settings, wall/floor and visibility breadth, measurement text, source glyph/AA pixels and hardware remain required beyond this bounded stage. Existing billboard filtering in the shared ray index requires further audit. Earlier iso and two-point precision failures, fixture hosting/window-name mistakes and stale-build failure are retained without relabeling them as passes.

## Timer-overrun followup

`probe-python-navigation-timers.py` starts the frozen real Walk/FirstPerson QTimers through actual handlers, blocks the same Qt event loop for170ms, and processes one event dispatch. Both timers emit once. The old compiled native egui controller replayed five Walk/seven FirstPerson fixed steps; raw failures remain under `navigation-timer-coalescing`. Deadline scheduling now emits one due step, starts without immediate motion, requests remaining interval, and stops on release/focus/cancel. Fixed movement math is unchanged. This follows [Qt6.12 timer-overrun policy](https://doc.qt.io/qt-6/qtimer.html#accuracy-and-timer-resolution).

Replay strict named controls with `python3 scripts/check-navigation-timers.py ORIGINAL.json FIXED.json OLD.json`. Both compiled old defects and a missing mandatory case must reject. This establishes controlled dispatch semantics, not rendered wall-clock cadence or complete Walk/FirstPerson parity.
