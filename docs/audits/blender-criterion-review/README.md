# P30 finite criterion reconciliation

P30's unchanged criterion is: “Render a retained fixture in actual Blender; camera/material/lighting consistency, cancel, errors and output verification.” The [retained actual Blender consumer](../blender-document-follow/README.md) satisfies it: eleven real renders, both engines, exact camera restore, PNG Save, cancel/reap, failed spawn preserving the prior image, and unchanged model/history. Five light cases/eight source/native states and two fractional camera cases match. Both 128×96 source/native frames are byte-exact after the retained rich GLB correction.

Independent review authenticated all17 application archive records and the immutable prior source archive, then decoded the two complete images. Root reviewed the actual consumer receipts, criterion and limits. No new render, build or test was necessary. The earlier batch README's broad “No full feature row” statement is superseded only for this finite row criterion by this disposition.

Full viewer/control workflows, textured production scenes, Windows descendant cleanup, Mac discovery, GPU utilization and peak memory remain open. All five release gates remain open. This is the optional Blender bridge criterion; native viewport rendering continues directly through Vulkan/Metal.
