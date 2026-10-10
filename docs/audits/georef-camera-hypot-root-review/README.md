# Source geographic framing norm correction

Whole actual original import handler18states across KMZ polygon, compound GeoJSON and local line, each new/seeded datum and Undo/Redo, now exact. Native saves reopen through original IGZ loader with matching captures. Prior seeded KMZ wrong distance14359.751935279943 is rejected; retained in ../georef-full-handler-root-review/kmz-failure.log. Correct source distance14359.75040282016.

Geographic framing now computes scaled f32 three-argument hypot with the source operation order instead of glam unscaled dot/sqrt. Official Qt qHypot delegates its three-argument overload to std::hypot: https://raw.githubusercontent.com/qt/qtbase/dev/src/corelib/kernel/qmath.h . Local libstdc++13 cmath __hypot3 implementation and actual pinned Qt QVector3D runtime confirm scaled float operations; production Rust has no C++/Qt dependency. No case-specific correction or tolerance change.

Combined542workspacePASS/twoignored. This proves these source Linux Qt numeric controls only, not arbitrary platform math-library equivalence. Geographic controls/schema/imagery/rendered input and full parity remain open; candidate isolated. Distant source tile-enumeration resource failure remains retained separately.
