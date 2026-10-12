# Survey CSV/datum application workflow

Frozen original `6be29fe437a7cd992d4e079f71720821b25216b9` supplies 46 states from actual SurveyPointsPanel Import/Clear handlers and eight ancestor-command states through actual Viewport nested edit/Undo/Redo. Only file-picker and zone/hemisphere answers were substituted. The authoritative inventory is exactly 54 states; no rendered/GPU claim. `source-manifest.json` authenticates every byte in `source-records.zip` and both capture helpers. These are newly missing survey UI observations, not recaptured accepted DXF/annotation data.

## Contract and runtime

A bare survey CSV uses P,N,E,Z[,description], accepts commas/semicolons/whitespace and quoted CSV descriptions; comma decimal cells are accepted with semicolon delimiters. Invalid/header lines are skipped; a file with no parsed rows reports an error. UTF-8 replacement decoding follows source. Without a datum, two cancellable source questions request UTM zone (default18, range1..60) then hemisphere (defaultSouth). With an established datum those questions are skipped. Local positions narrow through the source QVector frame while pre-existing native double geometry is untouched. The imported batch creates one command; a newly anchored datum belongs to that command. The camera fits only imported points with5m padding and remains live through subsequent Undo/Redo. Model units are unaffected.

Clear is a single command restoring ordered points on Undo; empty Clear is a no-op. The real tray displays actual point count. File Import Survey and the terrain tray use the same cold controller/modal. The pending rows retain no Scene clone and execute no per-frame parsing or persistence. Acceptance prepares finite positions/camera and reserves point capacity before recording/publishing. Existing snapshot history still owns one full Scene copy per command. A runtime-only survey marker carries global points/datum through ancestor-command Undo while nested geometry stays in its active frame.

## Integration replay (native acceptance pending)

Authenticate/extract `source-records.zip` into a private directory preserving flat input names. Required files include `original-survey.json`, `original-survey-context.json`, `comma.csv`, `semicolon.csv`, `bad.csv`, the three `*-input.igz` documents and all four `*-original.igz` saves. Run the jointly built `context_document_lifecycle <dir> --survey-points`, then pinned Python `scripts/reference-survey-reopens.py <dir>`, then `python3 scripts/check-survey-points.py <dir>`. The strict checker requires all54 states, four distinct ordered actual-original reader pairs, native-double/unknown-document/nonfinite guards and meaningful history/point mutations. Root performs the consolidated build; no owner build is duplicated.

Focused real-egui test: `survey_points::tests::actual_survey_modal_cancel_and_accept_owns_input`. It covers background Delete/CtrlZ while modal, Escape, zone/hemisphere Cancel, actual clicked acceptance and Undo/Redo. Native consumer guards refuse nonfinite coordinates before document/history mutation; source can admit some nonfinite CSV scalars, so this preservation boundary is explicit.

## Separate P27 scope

This is the previously missing survey import/datum/clear UI workflow. The existing path import/math/schema remain separate accepted data. A complete elevation profile is not claimed: source uses a loaded visible photogrammetric survey before DEM, absolute altitudes, incomplete footprint gaps and an actual profile dock/CSV export. Native lacks that sampler/dock owner; terrain-grid interpolation is not substituted for it. DEM/basemap networking, photo-survey import/render and the full P27 criterion remain open.
