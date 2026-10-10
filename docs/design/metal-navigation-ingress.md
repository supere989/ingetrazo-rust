# Native Metal motion modifiers

The native Metal host now records modifiers immediately after each egui-winit event append and swaps the bounded timeline when it consumes raw input. Backpressure retains both the input and its pending timeline. The existing navigation consumer rejects missing snapshots and poisoned timelines.

The existing controlled native-window probe is now built on macOS as well as Linux/Windows. The Mac SDK CI step compares its two controlled cases with the retained frozen Python observations and executes the missing-snapshot refusal control. This exercises egui-winit on an owned native window, not physical input or real OS event coalescing. Native runtime acceptance is pending until those jobs complete. Full navigation, physical Mac and viewport acceptance remain open.
