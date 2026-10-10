# First native failure followup

Frozen ARM input: run38084381325, source6c4d38cd; artifact11681589175 SHA25693d7a9b767fb93bb9422680896b23a10331dc74a7bf12dba95d618d9357ff230. Four face cases match; Times refuses generic resource acquisition and CJK reports PingFangSC-Regular not found in the collection. These failures remain retained in root evidence.

The candidate resolves a sparse cascade descriptor family through an instantiated CTFont when its family attribute is absent, following Qt getFontDescription's resolved-font family source. Additional style-name and selected-file errors identify the acquisition boundary rather than reporting a generic null.

Named native instances no longer require their PostScript name to equal the stored physical face name. If that lookup fails, exact CTFont copied head+name tables identify the physical collection index. Different metadata rejects; multiple matching faces reject as ambiguous. The fallback reads only bounded directory/metadata bytes, with a16MiB aggregate metadata-read limit; it never reads hvgl or raises the font file cap. Existing native variation/trait/matrix instance identity remains separate from the physical index.

Focused standalone Rust identity tests:3PASS including exact named-instance metadata, changed-head rejection, actual duplicate-offset ambiguity rejection, outline-read exclusion and malformed oversized name admission. No cargo rebuild or cloud launch. Native SDK execution, actual Times synthesis, source CJK descriptor selection, metrics/layout/raster/PDF comparisons are pending; strict face checker unchanged.

## Review correction

The table fallback now tracks distinct required head/name bits and rejects duplicate relevant tags, even if their lengths or bytes do not match. A malformed duplicate-head/no-name collection reproduces the former false-identity risk and now refuses. PostScript lookup exposes an Option match result: only a fully inspected no-match permits metadata fallback. Read failures, malformed records, allocation errors and budget refusals propagate without a second scan. Focused standalone tests now4PASS; no native SDK claim.

## Virtual-face physical resource candidate

Second actual ARM run38084954180/source e427d9bd now matches CJK plus the prior four cases. Times remains a selected-font URL refusal. Retained artifact11682550622 SHA256e8ddf52be292c4051bf49d2faf99d7e02d3a3dd9f2d24fac592f428c1d875a7e records source .Times Fallback with stored TimesNewRomanPSMT physical tables, Regular OS/2/head traits, effective bold and Normal style.

When selected CTFont lacks a URL, the candidate resolves stored PostScript names from its already-owned compact native SFNT through CoreText. It accepts the candidate descriptor URL only after exact head+name bytes match the selected virtual CTFont. It keeps the selected virtual CTFont for metrics/outlines/raster; the physical descriptor is only a temporary identity/resource witness. No request alias, family or file name is hardcoded. Up to16 unique identities and256KiB name metadata; no font inventory scan or full collection read. URL-bearing paths retain their current fast path.

The narrow ARM Rust metadata/FFI compile passed with a minimal ttf-parser API adapter, not an actual SDK/runtime check. An initial adapter returned raw_face by value instead of the real borrowed API and failed borrow checking; this adapter failure is retained, then corrected. The source file did not need that change. Full native SDK/runtime, exact selected resource, synthesized rendering and actual pixels remain pending coordinated batch. No cloud launch or broad build from this lane.
