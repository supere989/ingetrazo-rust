# Native default paper font repair

The first actual original sheet text witnesses from both macOS runners select `.AppleSystemUIFont`; the previous Rust provider forcibly mapped the same `Sans Serif` request to Helvetica. Both images differ at 2,025 pixels while their physical image headers match. The first native witnesses, font table identities and failures are retained separately and authenticated here.

The provider now obtains the platform system UI font for an unavailable family, enumerates its UI-design variants with the existing source style ranking, then falls back to family or the retained descriptor as Qt does. Registered named families retain their existing matching route. CF owners remain scoped and the existing request/font caches retain their warm path. No private system font name or fixture pixels are hardcoded into production matching.

The implementation follows pinned [Qt 6.12 CoreText font database source](https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/gui/text/coretext/qcoretextfontdatabase.mm): default AnyStyle/System descriptor and theme variant registration. Retained source SHA256 `0d45c954db30f38d3f5c4312dd07f27cf30b73884915b0c6d49adad5e657f561`. Independent review accepted fallback order and ownership. The affected ARM Rust metadata check passed in 5.53 seconds; it does not establish Apple SDK or runtime acceptance.

The dedicated native run reuses each architecture's first source archive. It compares seven actual selected-font metadata tables and ascent/descent, then compares the complete physical text image without tolerance. Its probe verifies warm ownership without another font match. No original capture, Linux source replay, or broad local suite accompanies this repair. Native pixel acceptance remains pending.
