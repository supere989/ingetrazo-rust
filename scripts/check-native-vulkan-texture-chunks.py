#!/usr/bin/env python3
"""Actual chunk-boundary/partial pixels plus frozen full-image admission failure."""
import argparse,re,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('image');p.add_argument('full_image_failure_log');a=p.parse_args()
text=subprocess.check_output(['identify','-format','%[pixel:p{100,60}] %[pixel:p{100,160}] %[pixel:p{100,260}] %[pixel:p{360,260}]',a.image],text=True)
actual=[tuple(map(int,v.split(','))) for v in re.findall(r'srgb\(([^)]*)\)',text)]
expected=[(255,0,0),(0,255,0),(0,0,255),(255,255,0)]
assert actual==expected,(actual,expected)
assert actual!=list(reversed(expected)),"Swapped-band negative control accepted"
log=Path(a.full_image_failure_log).read_text()
assert '48MiB image with 8MiB staging headroom must upload' in log and 'allocation 50331648 exceeds application budget 58720256' in log,"Old full-image uploader failure was not retained"
print('PASS actual texture chunk bands/partial pixels',actual,'and old full-image allocation failure control')
