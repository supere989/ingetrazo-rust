#!/usr/bin/env python3
"""One source constructor trace; no App/controller/frame recapture."""
import math,sys,json
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from ezdxf.math import bulge_to_arc,ConstructionEllipse,cubic_bezier_from_ellipse
center,start,end,radius=bulge_to_arc((0,0),(2,0),.5);s=start%math.tau;e=end%math.tau
if s>e:e+=math.tau
ellipse=ConstructionEllipse.from_arc(center,radius,(0,0,1),math.degrees(s),math.degrees(e));curves=list(cubic_bezier_from_ellipse(ellipse));span=ellipse.param_span;step=span/len(curves);angle=ellipse.start_param%math.tau
print(json.dumps({'center':list(center),'radius':radius,'raw_start':start,'raw_end':end,'normalized_start':s,'normalized_end':e,'ellipse_start':ellipse.start_param,'ellipse_end':ellipse.end_param,'span':span,'step':step,'final_accumulated_angle':angle+step+step,'sin_final':math.sin(angle+step+step),'cos_final':math.cos(angle+step+step),'last_cp':list(curves[-1].control_points[-1])},indent=2))
