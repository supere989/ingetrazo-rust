# Outline arena final bounded proof

GPU binaries were built from be38c1ea. Combined source 740ec97f merges main a1f25b2b; relevant-source-diff.patch is empty. This reuses unchanged GPU evidence after that navigation-only merge. One combined workspace suite passed 626 tests, with two ignored.

Actual NVIDIA RTX 5090 owned-window captures match all 29 frozen original Qt OpenGL image/outline frames exactly: 13,920,000 RGBA pixels. The existing strict checker is unchanged. Generic opaque ordered overlap matches two whole frames, with both invalid opaque admissions refused.

The two-chunk arena control preserves the entire old frame and old pose after real budget admission denial on the second changed chunk, after the first chunk completed two real transfers. Restored budget permits exact retry. One changed chunk requires two uploads totaling 72 bytes, with zero additional mapping/allocation. More than twenty warm frames add zero uploads, submissions, mappings or allocations. The literal old desired-pose route fails the same checker with “Old pixels changed after later candidate refusal”; its source patch, build log and captures remain retained. Normal and rejected clients close with zero committed ledger charge.

One callback and inline render pass preserve source order. Warm upload work is dirty-only, but prepare/use marking and draw traversal remain O(visible slots); result.json reports examined slots, callbacks, binds and draws. Logical admission is 32 MiB each for vertex/index outlines (64 MiB combined), plus actual Vulkan allocation padding and retained staging. This is an outline-specific cap, not a native GPU memory ceiling. Larger-scale admission, fragmentation, retained empty pages, total-path performance, arena-specific unknown-completion injection and native Metal arena consumption remain open.

Binary SHA256:
- lifetime: e920780ff92c03f10d7675acac884504a9c8d81a4e7ffec29430e55f14311b17
- old-pose negative: 98328c4e26dfeb7c183c6f0e63abb23899c6baef102ccfae6ef2ba72bfe61d1e
- opaque: b426658e72e881b787f1807a09077974d9c89bbce99198c2624bf3cf9be1dae8
- outline: 942dacc8a0045d3f423c8668cfa9457cdccb445c3b54ecd1846c5af5fe90e571

Transport used read-only GPU CDI, owned Xwayland client socket/auth, immutable binaries/input and writable disposable output. Only exact newly-created fixture titles were captured; no desktop capture. Temporary request/state files are excluded from committed evidence.
