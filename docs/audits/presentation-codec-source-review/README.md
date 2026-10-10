# Source presentation parser correction

Source `ad13b1b3`; pinned original `6be29fe437a7cd992d4e079f71720821b25216b9`. Twelve literal actual `from_dict` cases match output/refusal exactly. Eight negatives include the retained previous parser baseline. Full source tuple/name representation survives unrelated typed edits and runtime serde; optional back RGB validates every element before slicing. Source integer strings refuse decimal syntax, accept bool and valid underscore/sign/whitespace syntax; large JSON UTC values retain their normalized integer. JSON insertion order is preserved for Python tuple(dict) conversion.

IGZ load uses the strict parser and rejects malformed source tuples/UTC instead of silently defaulting. GPU-facing typed RGB is separate from preserved normalized source representations, with no rendered malformed-data claim. Main panel/save/view protocol and all16 context states rerun unchanged. Workspace 545 passed, two ignored.

Replay `scripts/check-presentation-codec.py original.json native.json ../presentation-codec-literal-baseline/baseline.json`. Unicode decimal digit syntax, nonfinite Python values/arbitrary integers beyond the supported JSON number range, and malformed saved-view recall exception/partial-mutation semantics remain explicit codec boundaries. This is not full style/shadow renderer parity.
