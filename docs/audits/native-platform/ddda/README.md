# Integrated native dimensions and selection platform run

Published runtime `ddda1ee2`, full [run 38150988828](https://github.com/supere989/ingetrazo-rust/actions/runs/38150988828). Native Windows MSVC compile, CLI and isolated recovery passed. Both Apple cross-compiles passed. Actual ARM and Intel SDK GUI builds passed, then both default World probes failed at runtime MSL compilation: the array declaration attached `clip_distance` to the array type. The new annotation and original-document startup steps were not reached. The failure is retained verbatim in `published-ci.json`.

Root repair `f3fd419e` uses the single-plane scalar `float clip [[clip_distance]]` in both output structs and all five vertex writers. Independent source review checked Apple MSL section 5.2.3.3: scalar float is an allowed output type. Actual corrected native runtime compilation and GPU acceptance remain pending. No logical clipping-sign change, extra pass or fragment discard was introduced.

Both ordinary ordered CAD compute jobs in [run 38150988832](https://github.com/supere989/ingetrazo-rust/actions/runs/38150988832) passed. They do not include the new World/annotation shader source and do not prove this repair.

`artifacts.json` authenticates the original unmodified Windows and two native Apple job artifacts against GitHub REST bytes and SHA256. The complete Linux workspace and semantic pipeline also passed, including the integrated seven-consumer App replay and native Vulkan consumers; no local broad suite was repeated. The run fails only at the two actual Apple MSL compilations.
