# Independent direct Vulkan image review

The reviewed candidate shares existing texture cache ownership with face rendering and submits a shader-generated six-vertex image quad directly. Membership/view changes rebuild descriptors; restricted pose publication refreshes frame pushes without geometry/image transfers. This remains an isolated review branch.

Review found that viewport control dispatch retained an outer geometry revision increment after the dispatcher had already published the same change. The redundant outer increment is removed to avoid duplicate composer/cache invalidation. Focused production revision tests are retained alongside this record.

Worker RTX evidence covers16cases/80sampled pixels, zero warm geometry/image transfers/hash/decode, real fence refusal, budget rollback preserving previous rendered pixels and ledger, retry and fenced retirement. This bounded sample evidence does not prove whole-image parity:15whole-image comparisons retain1212-3004differing pixels, including missing dashed outlines and central interpolation differences. Full native image rendering adoption is pending source-backed corrections and independent rendered acceptance.

Metal parity, mixed Rotate/Scale, arbitrary placement, image commit/Undo efficiency and all fullrelease gates remain OPEN.

Independent software replay completed all16case ownership/warm-transfer controls; all16retain zero warm uploads/hash/decode. Onlyfive sample families match the stored NVIDIA originals; elevensample families differ and remain retained. This is software-rendering evidence, not matched-device hardware acceptance. Actual physical-device name/vendor/device/type and explicit software request are now emitted by the probe to prevent misattribution.
