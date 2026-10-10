# Geographic map-block loader correction

Fresh actual original/native IGZ loaders match all twelve retained baseline cases. Twelve actual native saves reopen in original with equal map capture metadata. Invalid map recipes remove only map; empty blocks are ignored. Missing fields use source defaults; preset identities select original provider metadata; custom recipes preserve strings, finite numeric conversion, opacity clamp and empty patch fallback. Metadata zoom is independent of actual original control clamp1..21; imported coverage still uses its bounded actual requested zoom.

Historical failures remain in tile-capture-loader-baseline. This scope does not close source nonstring provider fields, Unicode/underscore numeric syntax, nonfinite/arbitrary precision values, exotic iterable patches or all codec limits. Fetch/cache/quads/GPU/broader UI and geographic parity remain fully required. No row or release gate promoted.
