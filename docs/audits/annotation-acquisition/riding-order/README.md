# Source ordering correction

Independent review found a real blocker in204f6f69: empty-selection dimensionhits cleared pickedgeometry before the both-anchors-riding classifier. Source move.py316–328 first clearsgeometry for labelhits, then gathers/classifies dimensions against remaininggeometry; only an effective non-riding dimension clearsgeometry. The correction follows that order before copy assets and snap exclusions are captured. Label-hit policy is unchanged.

One missing actualoriginal anchored-edgecase adds7phases. Bothdimension endpoints bind to the pickedfaceedge with zerooffset; originalmoves theedge, and thedimensionrideswithout an extraoffset. Newstate records include raworderedmeshvertexpositions through preview/Cancel/commit/Undo/Redo. Prior42source states are byte-value-identical, copied from the canonicalarchive without recapture. Old77corpus is untouched.

Also fixes the independently reviewed labelposition cancellation seam: narrowanchor and offset individually beforeadding, matching source QVector3D.position and the rendered LeaderInput. Authoredf64 fields remain untouched.

Source-only validation completed; correctednativeexecution deliberately deferred to root's one affected consumer+CLI build. Extractsource-records.zip and run correctedannotation_acquisition_fixture DIR, then scripts/check-annotation-acquisition.py DIR. The checker expects49states and verifies all49mutations; it also substantively rejects retainedwrongcamera42candidate against original42prefix. Canonical previousrecords.zip and its failures remain unchanged. No localbuild/capture ofexistingcases or broadsuite.
