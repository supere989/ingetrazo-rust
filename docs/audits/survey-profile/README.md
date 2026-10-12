# PhotoMesh elevation profile

The new native Profile action samples an original IGZ photogrammetric asset along the selected (or sole) GeoPath, shows the source statistics/gaps and exports the original CSV. It leaves Scene, history and document dirty state unchanged. DEM sampling remains unavailable and is explicitly refused when no visible PhotoMesh is usable; this batch does not substitute the terrain grid.

## Frozen source contract

`source-records.zip` and `source-manifest.json` authenticate nine original cases, eighteen actual ProfileDock compute/repeat observations and six CSV files. Source pin is unchanged `6be29fe437a7cd992d4e079f71720821b25216b9`, NumPy 2.5.3. Source observations cover 628 sample rows: sloped/gapped paths, closure, selected-path priority, overlapping highest-hit surfaces, vertical triangles, hidden survey layer, ambiguous/no-path/no-datum, empty survey and zero XY span. Survey visibility is authoritative; hidden layer alone does not suppress source height sampling. `dem-boundary.json` records the actual original PhotoMesh-first/DEM selection without initiating network access.

## Consumer and first-build commands

Extract authenticated source members into an isolated output directory. Run the prebuilt `context_document_lifecycle <directory> --survey-profile`, then from this checkout `/tmp/ingetrazo-parity-python/bin/python scripts/reference-survey-profile-reopens.py <directory>` and `python3 scripts/check-survey-profile.py <directory>`. The checker requires all nine ordered actual original-reader pairs, byte-exact six CSVs, decoder reuse and three unchanged-owner refusal guards and equal-version Undo/Redo refresh. Strict sample/statistic/history mutations are rejected. No native execution has been claimed at this prereview stage.

Focused tests are `photo_profile::tests`, `path_identity_tests::path_identity_is_runtime_only_and_clone_preserves_owner` and `survey_profile::tests::actual_profile_panel_is_readonly_cached_and_refusal_retains_owner`. Source incidence-unavailable behavior is distinct from configured budget refusal; both are directly tested. No GPU or Qt layout/pixel acceptance is claimed.

## Ownership and costs

A skipped runtime GeoPath token preserves the source held-object identity through clones/history; independently recreated or loaded paths receive fresh tokens, without changing semantic equality or disk schema. Immutable decoded arrays and a lazy index are shared by Arc. One cold explicit action hashes compressed bytes to establish asset identity; warm panel frames/path refresh do not read assets or hash them. Repeated same document/member/content reuses one decoded asset. Installed egui 0.30 requires an owned Mesh in each frame Shape; the cached chart is reused for preparation, but handoff clones its small vertex/index vectors into frame-owned rendering data. A changed path copies only its points/result and prepared plot; no Scene clone/history snapshot occurs in the production profile action. The native test snapshots Scene solely to prove readonly behavior. A forward segment cursor retains literal station interpolation/prefix arithmetic and visits each segment at most once per sample pass; no extra prefix array is allocated. Its bitwise boundary/duplicate-segment test compares the existing source-law helper.

`INGETRAZO_PROFILE_CPU_BUDGET_BYTES` configures checked CPU array/result/plot admission (default 256 MiB, separate from GPU accounting). Capacity accounting includes admitted temporary index buffers and retained candidate/old owners, using try_reserve. It does not bound the entire process heap, allocator bookkeeping, compressed Scene asset storage or egui frame rendering allocations. The source 16,000,000 incidence limit yields unavailable heights, rather than a native allocation exception. Source unavailable-index outcomes are cached to avoid repeated reconstruction. Failed asset/index/result/plot admission retains the accepted result and owner.

Wider P27 DEM terrain/profile sampling, map services, path authoring and full original profile UI remain separate scope. This is the complete offline original PhotoMesh profile consumer, not a claim of complete P27 parity.
