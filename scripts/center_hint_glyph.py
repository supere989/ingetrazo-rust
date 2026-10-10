"""Frozen UI fixture glyph classification; arbitrary painted backgrounds are outside scope."""
def is_green(rgb):
 r,g,b=rgb[:3];return r<100 and g>100 and b<150
def center_hint_metrics(pixel,cx,cy):
 core=sum(is_green(pixel(x,y))for y in range(cy-2,cy+3)for x in range(cx-2,cx+3))
 outer=sum(is_green(pixel(x,y))for y in range(cy-8,cy+9)for x in range(cx-8,cx+9))
 return {"filled_center":core>=13,"core_green":core,"outer_green":outer,"core_samples":25,"majority_threshold":13}
