#!/usr/bin/env python3
"""Bounded world-fragment clipping controls; does not accept section fill."""
import argparse,re,subprocess
p=argparse.ArgumentParser();p.add_argument('cut');p.add_argument('reverse');p.add_argument('inactive');p.add_argument('disabled');a=p.parse_args()
def sample(path):
 s=subprocess.check_output(['identify','-format','%[pixel:p{580,500}] %[pixel:p{740,500}]',path],text=True)
 return [tuple(map(int,v.split(','))) for v in re.findall(r'srgb\(([^)]*)\)',s)]
def red(c):return c[0]>200 and c[1]<10 and c[2]<10
def blue(c):return c[2]>200 and c[0]<10 and c[1]<10
def background(c):return min(c)>150 and max(c)-min(c)<10
cut,reverse,inactive,disabled=map(sample,[a.cut,a.reverse,a.inactive,a.disabled])
assert red(cut[0]) and background(cut[1]),cut
assert background(reverse[0]) and blue(reverse[1]),reverse
for value in [inactive,disabled]:assert red(value[0]) and blue(value[1]),value
assert not(red(reverse[0]) and background(reverse[1])),"Wrong-normal negative control accepted"
print('PASS active/reversed/inactive/disabled native fragment clipping; section fill remains open')
