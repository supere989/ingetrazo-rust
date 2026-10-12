# Native device caller compile proof

The exact external-caller capability query now passes Apple-target Rust metadata checking through the backend protocol re-export. The first standalone attempt lacked the host proc-macro dependency search path; both attempts are retained. Library metadata also passes. This proves caller scope/type checking, not GUI linkage, device capability or native rendering.

Previous actual diagnostic attempts af1d6226 and801409f8 failed consumer compilation: missing trait scope, then unavailable transitive crate import. Their downloaded original artifact ZIPs remain at root staging and must be retained with terminal API evidence. No failed attempt is relabeled successful.
