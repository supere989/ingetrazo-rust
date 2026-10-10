# Actual drawable producer attribution probe

Existing same-runner Intel evidence shows rawQImage/Qtwindow20,51,89 versusMetalwindow21,51,89. Explicit sRGB candidate leaves this failure unchanged. Existing MetalGPUclearreadback was a separate offscreen target, so it does not establish the actual drawable’s stored bytes.

This explicit diagnostic selects framebufferOnly=false before the first drawable acquisition, allocates one queried/budgeted256B shared buffer per admitted probe frame, and encodes a one-pixel center BLIT from the ACTUAL BGRA8Unorm drawable after rendering in the SAME command before presentation. No extra image or render pass is introduced. It reads four bytes only after actualcommandCompleted. The buffer is retained inside the immutable Frame through unresolvedpresentation/quarantine; reading a GPU sample does not authorize ownerretirement. Latest sample/commandidentity is fixed-size state, emitted to the own-window snapshot. Sourceformat is strictly checked.

Default production never enables this path: framebufferOnly stays true and there are no readback allocations/copies. Diagnosticselection after any submitted frame is refused. Native capture uses only the same own NSWindow ID and requires a validated actual GPU sample; existing pixel/presentation/resize/15s criteria remain unchanged. Unsupported/capturefailures still let the independent native gate finish.

BothMacalltarget crosschecks and three real child capture-refusal plus fast-capture acknowledgment controls PASS. ActualnativeGPU/WSI execution is pending; no causal or physicalMac claim.
