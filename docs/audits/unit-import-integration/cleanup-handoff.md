# Unit import disposable cleanup handoff

Both canonical archives and every manifest member were authenticated before inventory. Only the eight explicit directory roots in `cleanup-handoff.json` are candidates: original generated cache/data/settings, two native picker/config pairs, and the wrapper source extraction. Every extraction file matches its canonical source member. Every candidate file has an exact path, SHA256 and byte size. No deletion has occurred.

Root review command: `python -m json.tool docs/audits/unit-import-integration/cleanup-handoff.json`. Before an approved purge, rehash every listed file, reject symlinks and any unlisted file under each root, then delete only the listed directory roots. Recheck canonical archive digests afterward and record the exact removal receipt. Do not use a wildcard over `/tmp`.

Raw native/source observations, failures, IGZs, diagnostics, logs and receipts remain protected, together with canonical archives, worktrees, original Python and the shared Cargo target. The inventory does not authorize removal of any other Unit or older units-workflow directory.

Root completed the exact reviewed purge after reauthenticating both archives, every member and every disposable file, rejecting symlinks and unlisted files. [Removal receipt](cleanup-receipt.json) records eight directories, 47 files and 149,763,071 logical bytes; canonical archive hashes were checked again afterward. Physical reclaimed bytes were not measured.
