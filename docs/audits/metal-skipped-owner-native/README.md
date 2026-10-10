# Native ARM presentation failure after skipped-owner correction
Revision00222ed, run38048129416, ARMjob114201685511/artifact11668896212: actual GPU completed ten submissions; seven actual zero-time presentation callbacks retired as skipped. Displayed counter remains zero. Front frame7 remains GPUCompleted without callback, while8/9 have zero-time callbacks; three unresolved frame owners remain quarantined.
Visible800x600 layer/view/drawable geometry contradicts an empty-layer explanation. The exact15second protocol fails and does not count skipped frames as visible success. Further Cocoa/presentation ordering diagnosis required.
Intel job114201685374 succeeds independently; no both-platform host or physicalMac claim. Existing mip/draw protocols in ARM artifact remain separate.
