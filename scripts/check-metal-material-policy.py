#!/usr/bin/env python3
"""Bounded actual draw-policy/front_facing receipt; never waives strict frames."""
import copy
import json
import math
import sys
import zipfile
from pathlib import Path


def validate(value):
    assert value["depth_compare"] == "less_equal"
    assert value["sample_count"] == 4
    rows = value["operations"]
    assert isinstance(rows, list) and 0 < len(rows) <= 65536
    for row in rows:
        assert isinstance(row["reflected"], bool)
        assert row["winding"] == ("clockwise" if row["reflected"] else "counterclockwise")
        assert isinstance(row["prototype"], int) and row["prototype"] >= 0
        draw = row["draw"]
        assert draw["kind"] in ("opaque", "default_back", "color", "texture")
        assert draw["cull"] in ("none", "front", "back")
        assert isinstance(draw["depth_write"], bool)
        if draw["kind"] in ("opaque", "color"):
            assert isinstance(draw["count"], int) and draw["count"] > 0 and draw["count"] % 3 == 0
            assert draw["first"] >= 0 and draw["first"] % 3 == 0
        if draw["kind"] == "color":
            assert draw["phase"] in ("DefaultBack", "TransparentFront", "OpaqueBack", "TransparentBack")
            assert math.isfinite(draw["opacity"]) and 0 <= draw["opacity"] <= 1
            assert len(draw["first_color"]) == 4
            assert all(math.isfinite(x) and 0 <= x <= 1 for x in draw["first_color"])
            if draw["phase"] == "TransparentBack":
                assert draw["cull"] == "front" and not draw["depth_write"] and draw["bias"] == [1, 1]
    return value


def main():
    source, output = map(Path, sys.argv[1:])
    with zipfile.ZipFile(source / "records.zip") as archive:
        original = json.loads(archive.read("complete-config/original.json"))
    report = []
    for case in original["cases"]:
        name = case["name"]
        policy = validate(json.loads((output / f"{name}-material-policy.json").read_text()))
        pixels = (output / f"{name}-front-facing.rgba").read_bytes()
        assert len(pixels) == 800 * 600 * 4
        front = sum(pixels[i:i+4] == b"\x00\xff\x00\xff" for i in range(0, len(pixels), 4))
        back = sum(pixels[i:i+4] == b"\xff\x00\x00\xff" for i in range(0, len(pixels), 4))
        assert front + back > 0
        report.append({"name": name, "front_pixels": front, "back_pixels": back,
                       "scope": "Actual shader front_facing with unchanged admitted cull/depth/bias; lines and MSAA boundaries are unclassified",
                       "policy": policy})
    seed = next(row["policy"] for row in report if any(op["draw"]["kind"] == "color" for op in row["policy"]["operations"]))
    negatives = []
    for kind in ("depth", "samples", "winding", "cull", "color"):
        mutated = copy.deepcopy(seed)
        if kind == "depth": mutated["depth_compare"] = "always"
        elif kind == "samples": mutated["sample_count"] = 1
        elif kind == "winding": mutated["operations"][0]["winding"] = "invalid"
        elif kind == "cull": mutated["operations"][0]["draw"]["cull"] = "invalid"
        else:
            next(op["draw"] for op in mutated["operations"] if op["draw"]["kind"] == "color")["first_color"][0] = float("nan")
        try: validate(mutated)
        except (AssertionError, KeyError, TypeError, ValueError): negatives.append(kind)
        else: raise AssertionError(f"Accepted mutated policy: {kind}")
    (output / "material-policy-check.json").write_text(json.dumps({"diagnostic_passed": True, "strict_pixel_gate_unchanged": True, "negative_controls": negatives, "cases": report}, indent=2) + "\n")
    print(json.dumps({"diagnostic_passed": True, "cases": len(report), "negative_controls": len(negatives), "front_back_pixels": [[r["name"], r["front_pixels"], r["back_pixels"]] for r in report]}))


if __name__ == "__main__": main()
