# Standalone Metal target admission correction

Apple documents `heapTextureSizeAndAlignWithDescriptor` as size/alignment **if created from a heap**, and describes its use to estimate a new heap. It does not specify an upper bound for `newTextureWithDescriptor`. Primary references:

- https://developer.apple.com/documentation/metal/mtldevice/heaptexturesizeandalign(descriptor:)?language=objc
- https://developer.apple.com/documentation/metal/mtlresource/allocatedsize

The former positive heap estimate was incorrectly used as the standalone allocation ticket. The corrected cold path reserves all available logical capacity (existing live/external/global/reserved accounting), then refuses zero actual size or actual size exceeding that ticket. It reconciles to the actual resource size under the same lock before publication. Existing malformed query refusal is retained. No guessed padding, budget increase, post-allocation ticket expansion, or allocator upper-bound claim is introduced. Both query-unavailable and positive-query paths now use the same standalone contract; the prior query-unavailable logical sample check remains unchanged.

The retained ARM failure reports neither actual nor ticket size, so zero versus over-ticket remains unresolved. The new error reports both plus heap estimate/alignment, descriptor format/samples/extent and device. This is a source-contract correction, not a claim of native success or proven observed branch. No build or cloud execution was performed; `git diff --check` passed. Existing standalone-capacity controls remain unchanged. Native runtime/strict section pixels remain required, including separately exposed transparent cap and background failures.
