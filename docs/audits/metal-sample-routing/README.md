# Consistent native world sample positions

The actual `67546fe7` ARM section run reached all ten captures after the target and stencil repairs. Nine strict images still differed (292,150,292,8,292,0,1034,1101,1034,2 pixels). A fresh section-only World never programmed the measured source sample pattern: the old route depended on dimensions or an earlier annotated frame. Those failures remain failures.

Runtime `d8c23b38aa7cfa63d4c21e52308d23b472ae8edc` applies the existing reflected NVIDIA source pattern to every four-sample SceneWorld frame on devices that advertise programmable sample positions. It removes the prior-frame annotation position cache. Device capability/default-pattern queries now run once at NativeMetal creation, not per frame. Single-sample frames retain no programmed pattern. Devices without programmable positions retain their default raster pattern; their strict annotation depth-selector admission is unchanged.

The section consumer records positions from the actual immutable Prepared frame and refuses a missing source pattern on a programmable device. Its ten-image checker and all original images are unchanged. No source pixel tolerance or universal Apple OpenGL resolve claim is introduced.

Independent read-only review found no blocking ownership/cache issue. One affected GUI Apple metadata preflight passed in 4.02 seconds using the existing exclusive target. This is Rust type checking for the section, billboard, and annotation consumers; it is not SDK/link/MSL/GPU proof. Actual Mac execution belongs to the focused review branch workflow and remains pending here.
