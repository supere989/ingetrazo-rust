# Root Metal clip review

Reviewed source `0fd81a11210fa26f2abcc5636beb41e1eea72dd6` retains six native lifecycle failures from the previous host revision. Unbounded egui clips are intersected with the physical framebuffer; vertex/index/UV and NaN validation remain in force. Finite endpoints retain prior f32 multiplication and rounding. The literal half-pixel control distinguishes the rejected f64 variant.

Both Apple architectures pass Metal all-target and GUI lib/tests/document-probe cross-checks. These are compile checks, not native rendering acceptance. Fresh native lifecycle execution remains pending; the isolated host is not adopted on main. All full parity rows and release gates remain open.
