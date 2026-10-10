# Genuine Rust interpreter feasibility

Standalone proof tools/ai-interpreter-proof pins rustpython-vm0.6.0 with default features disabled and compiler/gc/importlib/stdio enabled. It executes an actual Python program with classes/functions/generator loops, a registered Rust native module and persistent scope; the host follows eval-first then exec fallback, updates `_` only for non-None expressions, captures stdout and renders real interpreter exceptions.

Frozen6be29fe core.ai.run_transactional runs the same literal language program, expression, None, advancement, SyntaxError and runtime failure. The synthetic native_probe module uses a Python function in the reference and a Rust native function in the VM: it tests extension registration, not original CAD-module compatibility. Captured stdout/value/error type/script frame match. Full traceback bytes differ because original includes its Python transaction wrapper; this is explicitly not exact arbitrary traceback formatting acceptance. Five actual-validator mutations reject.

No live provider, user document or external messages. Scene/Qt/core adapters, full imports/stdlib, journal and production integration remain OPEN. RustPython is a genuine Rust interpreter, not CPython/process wrapping or regex recipe substitution. Language-only reference leaves model history unchanged; this is not a native atomic-document proof.

Run cargo run --manifest-path tools/ai-interpreter-proof/Cargo.toml with reduced-debug isolated target; generate reference with the pinned original Qt environment and scripts/reference-ai-interpreter-language.py. Compare retained result directory with scripts/check-ai-interpreter-language.py. No broad workspace suite is required for this standalone source.
