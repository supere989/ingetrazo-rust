# Windows native save and process recovery accepted

Published isolated repair `9b8e9091fda6ae989161761414962f512072d186` passes [Windows MSVC acceptance 38144420256](https://github.com/supere989/ingetrazo-rust/actions/runs/38144420256). The actual packaged CLI and process consumer compile together; CLI checks/version, all seven independent process/app recovery states, exact unchanged original-file bytes and 12 rejection controls pass. The existing temporary IGZ is reopened with write access before `sync_all`, satisfying Windows FlushFileBuffers without truncating it.

`records.zip` is the uploaded artifact, authenticated against the job-reported SHA256 in `receipt.json`. Earlier failing Windows and Apple artifacts remain at `../f901/`. This is actual Windows execution, not Windows GPU/window acceptance.

The repair branch ran only the Windows job, preserving all gates on the integration branch. The next integration workflow also retains compiled external dependencies when a semantic/platform check fails (`cache-on-failure`), while excluding workspace crate caches. [Upstream cache input](https://github.com/Swatinem/rust-cache/blob/v2/action.yml). No new source-oracle acquisition or broad local suite was needed.
