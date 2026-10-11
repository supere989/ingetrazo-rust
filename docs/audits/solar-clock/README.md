# Source solar clock

The native Rust solar clock follows pinned original `6be29fe437a7cd992d4e079f71720821b25216b9` `core/sun.py`. This is the clock/geographic prerequisite for P20, not rendered shadow acceptance.

198 fresh original/native states compare exactly, including all floating-point values without tolerance. The frozen protocol covers seven latitudes, four seasons, six UTC times, polar day/night, half-even longitude zones, leap/century/year rollover, invalid dates/offsets, rotated north, the default site and disabled shadows. Six corrupted-output controls are rejected. Two focused Rust tests pass. No workspace suite or GPU capture was run for this core-only checkpoint.

`initial-fixture-failure.zip` retains a diagnostic error: the first fixture incorrectly dropped the preserved out-of-domain UTC offset when invoking the separate daylight utility. The production clock correctly refused it. The fixture now checks daylight refusal separately from polar night; math and acceptance tolerances did not change.

Replay the original with `scripts/reference-solar-clock.py --source <pinned original> --output <directory>`, pipe `input.json` through the core `solar_clock_fixture` example into `native.json`, then run `scripts/check-solar-clock.py <directory>`.

The integrated Render tray now exposes date, annual study, local time, daylight range, darkness and UTC zone. Eight actual original ShadowsPanel apply states match the native document controller exactly. Two focused GUI tests pass, including actual egui repaints that preserve loaded night time, non-rounded brightness and an offset outside the dropdown. Opening the panel does not rewrite document data. Daylight range is cached by site/date/zone.

The rendering helper reuses the existing native local calendar provider, honors disabled/sheet/kill-switch policy, and uses the scene datum/north. The source panel intentionally uses a fixed 2026 day-of-year/daylight study while the renderer uses the current local year; this split is preserved. No external process or added dependency is used.

Vulkan/Metal shadow pixels, complete panel gestures/visual fidelity and the location picker remain OPEN. These core/controller checks do not claim rendered shadow acceptance.
