# Physical sheet pictures

The original source corpus is frozen at `6be29fe437a7cd992d4e079f71720821b25216b9`: eleven actual workflows cover Add Image, JPEG/stretch, cover/contain, rounded borders, ellipse/rounded feathering, opacity, hidden and missing images, and same/higher-z overlap with shared assets. Each case retains source PNG, JPEG, PDF and IGZ output, with original save/reopen equality. Source paths remain literal external `assets/...` references; the IGZ writer does not embed these picture bytes. Source image rotation is not implemented, so no rendered rotation parity is inferred.

`source-records.zip` and `source-manifest.json` authenticate 48 members / 762,765 raw bytes, including three unchanged assets from the original repository. Root independently compared all members to the terminal capture and the three original assets. Native implementation and acceptance are **pending**; this source inventory is not a native parity result.

The pending native consumer contract is `sheet_picture_workflows SOURCE OUTPUT [--software]`. `scripts/replay-sheet-pictures.py` authenticates and extracts this frozen corpus, executes the supplied prebuilt consumer, then invokes the strict PNG/JPEG/PDF/persistence comparator. Both stages run with the source directory as their working directory so external paths preserve the original resolution rule. No source capture or Rust build occurs in replay. The helper is prepared but has not yet been executed against the unfinished native workflow.

The [independent source-only checkpoint](../sheet-pictures-independent-review/README.md) authenticates the archive, producer binding, original assets and unexecuted replay contract. It does not promote native acceptance.
