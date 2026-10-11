#!/usr/bin/env python3
"""Strict pinned-source job configuration comparison; no Blender execution."""
import copy
import json
import sys

def validate(original, native):
    if set(original) != {"jobs", "environments", "cache"}:
        raise AssertionError("unexpected original inventory")
    if set(native) != set(original) | {"refused"}:
        raise AssertionError("unexpected native inventory")
    if len(original["jobs"]) != 21 or len(original["environments"]) != 3 or len(original["cache"]) != 4:
        raise AssertionError("missing source cases")
    names = [row["name"] for row in original["jobs"]]
    if len(set(names)) != len(names):
        raise AssertionError("duplicate job case")
    if {k: native[k] for k in original} != original:
        raise AssertionError("source/native configuration differs")
    if native["refused"] != [True, True, True]:
        raise AssertionError("unsafe camera admitted")

def main():
    original = json.load(open(sys.argv[1]))
    native = json.load(open(sys.argv[2]))
    validate(original, native)
    mutations = []
    for field in ("jobs", "environments", "cache"):
        changed = copy.deepcopy(native)
        changed[field].pop()
        mutations.append(changed)
    changed = copy.deepcopy(native)
    changed["jobs"][0]["job"]["camera"]["eye"][0] += 0.001
    mutations.append(changed)
    changed = copy.deepcopy(native)
    changed["refused"][0] = False
    mutations.append(changed)
    for changed in mutations:
        try:
            validate(original, changed)
        except AssertionError:
            continue
        raise AssertionError("mutation accepted")
    print("PASS: 21 exact jobs, 3 environments, 4 cache cases, 3 refusals, 5 rejected mutations")

if __name__ == "__main__":
    main()
