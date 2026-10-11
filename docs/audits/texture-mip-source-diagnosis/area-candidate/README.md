# Direct GPU area reduction candidate

The isolated `review/texture-area` candidate dispatches cold area reductions in the existing Vulkan upload command. Actual source/native raw levels remain a strict failure: three assets differ by at most one channel value, while the 150×151 glass still differs by 57. The candidate is not enabled in production.

`evidence.zip` retains exact candidate source and bytecode, source reset inputs/results, build log and native GPU levels. `manifest.json` authenticates every member. Existing parent `assets.zip` supplies the unchanged original four asset levels. Source reset controls prove the reduction depends on current dimensions/data, rather than earlier mip-chain history. The even-axis shift and exact rounding rule remain unresolved. No offsets or tolerances were fitted.
