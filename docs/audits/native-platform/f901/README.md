# Production platform startup failures

Source revision `f90106ca2a74e21f6d9cca50d02324fce32ea1f9`, published acceptance [38143497282](https://github.com/supere989/ingetrazo-rust/actions/runs/38143497282). ZIPs are unmodified uploaded job artifacts; `provenance.json` records their byte identities.

Windows MSVC compiled the actual CLI and recovery consumer; CLI `--check` and `--version` passed. Recovery failed at its initial `save_igz` with Windows error 5 (Access denied). The temporary file had been reopened read-only for `sync_all`; Windows FlushFileBuffers requires GENERIC_WRITE. The repair reopens the existing temporary file with write permission, retaining atomic sibling rename and failure cleanup. [Microsoft FlushFileBuffers contract](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers). The isolated seven-state Windows recovery workflow is the acceptance check for this repair.

Apple ARM built and linked but the normal native CLI did not produce its world-submission marker. The Metal adapter categorically rejects normal default `show_profiles` and `show_sky` styles. Native implementations remain required; disabling defaults or weakening the marker is not an acceptance repair. The initial attachment log's 0x0 drawable precedes resize and does not diagnose the failure.

The `review/platform-startup` repair branch runs only the affected Windows job. Other branches, including integrated review, retain all platform and semantic jobs. No main-branch update is authorized or performed.
