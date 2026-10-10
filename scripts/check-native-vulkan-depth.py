#!/usr/bin/env python3
"""Frozen crossing-face depth acceptance and retained centroid failure control."""
import argparse,re,subprocess
p=argparse.ArgumentParser();p.add_argument("vulkan_image");p.add_argument("centroid_image");a=p.parse_args()
def samples(path):
 text=subprocess.check_output(["identify","-format","%[pixel:p{580,500}] %[pixel:p{740,500}]",path],text=True)
 return [tuple(map(int,v.split(","))) for v in re.findall(r"srgb\(([^)]*)\)",text)]
def passes(values):
 return len(values)==2 and values[0][0]>200 and values[0][1]<10 and values[0][2]<10 and values[1][2]>200 and values[1][0]<10 and values[1][1]<10
positive=samples(a.vulkan_image);negative=samples(a.centroid_image)
assert passes(positive),f"Vulkan fragment depth incorrect: {positive}"
assert not passes(negative),f"Centroid negative control unexpectedly passed: {negative}"
assert not passes(list(reversed(positive))),"Swapped-camera-side negative control accepted"
print("PASS crossing-face Vulkan depth",positive,"retained centroid failure",negative,"and swapped-side negative control")
