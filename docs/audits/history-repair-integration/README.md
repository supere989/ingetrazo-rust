# Reviewed history repair and complete finite Fillet criterion

Runtime `3e53a194fd00bb718fe946236c9b85310007f59d`, unchanged original `6be29fe437a7cd992d4e079f71720821b25216b9`. One cached CLI/five-consumer build took **10.15 seconds**. All five fresh application workflows pass against authenticated retained source inputs; the existing Fillet output also passes the independent volume checker without another application run or build. `manifest.json` pins binaries, scripts and all 70 retained records. The archive was byte-verified before temporary copies were released.

The reviewed shareback chain fixes the three actual bf85 Linux failures while preserving 28 Fillet and 33 Change Axes states. Source-style referenced geometry and curve repartition remain; native entity identities and opaque metadata survive, f64 coordinates remain precise under identity/translation, and malformed rings, nonfinite orphan input, conflicting merged metadata and UVW overflow refuse before publication. A separate read-only reviewer found the ring, UVW and identity-frame gaps in the first candidate; the followup resolves all three. Owner focused tests cover the original three unchanged assertions plus directly affected ownership controls. [Repair and retained failures](../shareback-regression-repair/README.md).

## P09 criterion disposition

The unchanged criterion is: “Generate proper rounded corners using radius/segments; topology, volume and undo tests.” It is now **Verified** for the finite accepted corpus:

| Required behavior | Accepted evidence |
| --- | --- |
| Radius and segments | Six retained original/native bodies cover radius 0.2 with eight segments and radius 0.1 with four segments; existing app controls cover typed and pointer sizing and retained settings. [Scoped source proof](../fillet-scoped/README.md) |
| Topology | Ordered source/native vertices, face loops/holes, curve partitions and attributes agree; independent opposite-edge incidence verifies each closed boundary. Corrected oblique and turned construction replaces the retained invalid source fixture. [Correction](../fillet-oblique/README.md) |
| Volume | Independent exact-rational boundary integration accepts all six freshly generated bodies against the source binary32 rounding cells. The simple eight-segment corner also agrees with the analytic discrete arc volume, 1.982428903045161. Five corruptions reject, including two topology-preserving volume changes. [Method](../fillet-independent-volume/README.md), `acceptance.json` |
| Undo and cancellation | Existing ten corrected oblique app phases and all 28 nested integrated phases pass, including Cancel, commit, Undo/Redo and child/outer Finish. The previous candidate and mutations reject. [Nested source proof](../fillet-nested-history/README.md) |

This disposition does not expand the finite criterion into arbitrary geometry or whole-application pixel acceptance. Those broader requirements remain under their existing rows and release gates. The source/native protected-data boundary is explicit rather than silently copying invalid or lossy source behavior.

The same joint run preserves the existing P01, P02/P03-related and Axes evidence: six complete original document reopens, five Axes reopens/33 states, six host-hole application cases and 65 persistent Push/Pull operations. P02's independent solid-boolean criterion proof is retained unchanged and is not rerun here.

`review/history-repair` runs the Linux semantic/workspace job and Windows native process job. It skips unchanged Metal jobs with known full-document section/dimension blockers. Full integration branches and pull requests retain their complete matrix. [Terminal bf85 platform results](../native-platform/bf85/README.md). Metal section/dimension implementation continues independently and will receive native Apple acceptance when complete.
