# Actual native failure: no consumer acceptance

Run 38055468236 source c1fe1a8fe9382313ca32e0d2b16329007511aebe.
ARM job114222897528 artifact11671420673: runtime Metal shader compiler rejects mismatched uint3/uint2 thread index declarations. Correction uses matching uint3 declarations and explicitly projects local.xy, preserving dispatch 8x8x1 and source tile arithmetic.
Intel job114222897681 artifact11672080533: strict RGBA8 read/write tier2 admission refuses this actual device. This is a required unsupported capability gap, not a passed/optional case. Apple tier1 supports R32Float/R32Uint/R32Sint only; RGBA8 is tier2. The strict seven-image/native consumer gate is unchanged. A separate exact Intel-compatible implementation remains required.

No native GPU success claimed for the correction until fresh runtime execution. Complete artifacts retained unchanged.
