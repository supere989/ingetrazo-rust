# Conditional production tile resolve

Source: 9302b810df84e44477b8a39de381bcecb469fb37.
Run: 38162764055, both jobs terminal failure. No retry was launched.
ARM job 114538952004; Intel job 114538951809.

The ARM SDK build, all-ten production encoder, defaults, billboard, document
annotations and packaged CLI probes passed. The strict checker failed only the
three retained isometric cases (134/152/134 pixels). Seven full images are exact;
no newly wrong pixels. Independent QImage decoding reproduced all ten counts.
All 74 listed source members authenticate against the frozen candidate.
The retained 4→1→4 control confirms one cold tile build, one-sample omission,
identical restored output, and intact old Prepared ownership.
Owner ledgers in default, billboard and document reports are 0/0.
The actual CLI assertion is world submission without opt-in flags; it does not
establish pixels, physical presentation or clean close.

The Intel SDK build succeeded. Defaults and billboard controls passed.
The first closed reference matches the prior native Intel frame exactly, but
the normal production candidate first frame contains 480000 zero RGBA pixels.
Its first differing warm frame was not saved by that fixture revision. Therefore
the cause is unresolved and warm/ten-case Intel acceptance is absent.
Document annotations and CLI reject the unprogrammable source sample policy.
No tile or diagnostic compute is selected for this normal Intel path.

Archive digests:
- ARM artifact 11710899262:
  2569145928864b000e07e9d44d46e59bacffb9a88e45755de870e26bda1ef784
- Intel artifact 11711092455:
  94762d055dbdc6b2158833d6234b082c197b246640ec3c324e147fdd8d06ba79

The small follow-up fixture patch records expected/actual raw warm bytes and
iteration, route, sample positions and counters only upon a mismatch. It changes
no production behavior and has not justified or launched another cloud run.
