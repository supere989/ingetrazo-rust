# Combined Vulkan images and dynamic fields review

Reviewed source d3b31c8a combines public field export4becda1c with independently reviewed image branch7f4f9ed7. Locked workspace584PASS/twoignored. Initial build reference symlink setup failure is retained; corrected frozen-reference link full rerun passes.

Exact combined probe independently rebuilt and run on actual NVIDIA RTX5090, read-only network-disabled GPU container. All24 complete800x600 RGBA image surfaces equal original source (11,520,000 pixels). Device metadata rejects3 corruptions; old inverse-V software frame and one-channel pixel negatives reject. Eleven warm poses per case have zero geometry/image upload/hash/decode.

First checker attempt refused a missing earlier software negative frame, which was untracked in the earlier worktree; retained failure and empty result are preserved. Earlier inverse-V llvmpipe frame/metadata are now retained self-contained, and complete strict checker passes. Earlier c8c4f8e9 commit title preceded checker completion and must not be treated as passing evidence by itself.

Outlines excluded explicitly; full outlines, mixed workflows, Metal and representative rendering/performance remain OPEN.
