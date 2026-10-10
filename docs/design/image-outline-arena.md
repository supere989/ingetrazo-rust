# Ordered image-outline GPU arena

## Base and boundaries

CPU journal/chunks a5708a0c merged with canonical 51940fd8 at 2b4ad9e9. Canonical navigation and both GPU fixture variants retained. No resources files changed. This document is design, not GPU acceptance.

## Publication

One callback consumes scene-order immutable plane chunks. Candidate slots reference vertex/index ranges inside paired pages. A dirty chunk reserves unused ranges; never overwrite a submitted or published range. Upload only the new vertex/index bytes. Prepare palette/order metadata and all ranges before publishing. Any allocation/admission/transfer failure discards the candidate and preserves old callback, buffers and ordering. Unknown GPU completion quarantines owners and accounting; it cannot recycle ranges.

A published snapshot retains page owners plus ordered ranges. Mark pages used with the actual next graphics serial. Replaced ranges enter a retirement list with last submission serial. Reclaim only after the production graphics fence acknowledges that serial. Page destruction follows existing Resources lifetime/poison rules. Free ranges are offset metadata, not a second memory ledger. Retained staging charge is part of Resources budget, not silently excluded from page admission.

## Transfer API dependency

Current Resources upload/upload_many return newly sized buffers. They cannot create capacity pages or scatter updates into existing fresh ranges. Needed owner-reviewed APIs: allocate_buffer(size,usage), upload_fresh_range(buffer,offset,data), and bounded upload_ranges batching. Validate bounds, usage and nonoverlap; keep candidate range ownership until transfer completion. No whole-page shadow copy or full-page reupload substitute. The root-owned persistent transfer staging work must finish review before this API is changed.

## Draw path

Use existing validated opaque CAD pipeline and palette semantics. One callback/pass. Iterate source plane then source edge order. Bind page pairs only when changed; draw indexed with base vertex and first index. Do not group by page if that changes source order. Opaque route requires actual coverage=1 and every used color alpha=1. Restore existing egui state through current callback contract. Native Metal must later consume the same immutable data/range descriptions using native allocations and command encoding.

## Required proof and counters

Cold pages, actual allocation bytes, logical live bytes, retired range bytes, page fragmentation, candidate rollback bytes, dirty upload bytes, transfer submissions/maps, callback/bind/draw counts, and CPU/GPU elapsed time. Warm identical view has zero geometry uploads. One varied pose only rebuilds/uploads that plane. Compare whole frames with frozen 29 original GL frames and old native controls; inspect source ordering with different colors. Force page boundary and failed later upload after earlier candidate work; retain prior full frame and actual ledger. Actual fence-held timeout must keep page owners charged and refuse admission. Many-plane measurements include transfer/draw, not only CPU journal timing. Total-model streaming and native Metal remain open.
