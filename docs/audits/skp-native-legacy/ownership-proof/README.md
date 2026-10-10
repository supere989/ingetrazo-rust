# Legacy archive ownership correction

Source 621f45f3 shares immutable archive objects through Arc; object references no longer deep-clone faces or loops. Root entity lists store unique IDs in first-encounter order, so repeated root references neither retain duplicate payloads nor repeatedly rebuild them. Candidate faces are cloned once per unique root identity.

A conservative 256 MiB decoded admission includes slot/tree/object bookkeeping, Vec growth, root identity bookkeeping, names, copied loop storage, and a second archive-sized allowance for candidate publication. This is an admission estimate, not an RSS or exact allocator measurement. The focused control verifies repeated loop references have the same Arc owner without incremental decoded charges and rejects budget exhaustion.

All six unchanged actual pinned legacy primitive comparisons pass on a freshly rebuilt binary. Real empty v17 application refusal and raw parser metadata remain distinct. This correction does not establish real nonempty legacy model, DIB, definitions, annotations, saved views or full era parity. The pinned source explicitly maps extended inline/by-object layer drawbase references to the default layer; this behavior is preserved.
