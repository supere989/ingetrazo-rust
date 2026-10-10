# Independent cached PDF font review

Fresh original Qt painter observations and original PDFs were regenerated from pinned Python6be29fe437a7cd992d4e079f71720821b25216b9.
Actual native production fixture outputs match all nine selected single-line pages at72/144/300DPI (27 whole-page comparisons exact); actual reader text matches nine cases, and same-size valid ToUnicode mutation changes reader text as required.

The fixture also executes real warm-byte/stat/subset Arc checks, source identity collision refusal, invalid glyph/nonfinite destination preservation, cache retirement and retained resource lifetime. Full combined workspace: 548 passed, 2 ignored.

Native Apple bundled font dependency build/library tests are now explicit CI gates. Pending native proof prevents adoption into main. Paragraph/bidi/fallback/underline/platform font resolution and Text export UI remain full required scope. Existing historical failed text renders remain retained; no full row or release gate promoted.
