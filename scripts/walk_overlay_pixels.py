"""Positive Walk cross/ring presence; not whole-image AA equality."""
def inspect_walk_overlay(image,baseline,rect,anchor):
 assert not image.isNull()and not baseline.isNull(),'unreadable image'
 x0,y0,x1,y1=map(int,rect);p=list(map(round,anchor))
 assert x0+12<=p[0]<x1-12 and y0+12<=p[1]<y1-12,'anchor outside world region'
 samples=[image.pixelColor(x,y)for x in range(x0+20,x1-20,20)for y in range(y0+20,y1-20,20)]
 assert samples and sum(max(c.red(),c.green(),c.blue())>30 for c in samples)/len(samples)>.9,'black/unpresented world'
 def seen(points,predicate):return sum(any(predicate(image.pixelColor(p[0]+dx+ox,p[1]+dy+oy).red(),baseline.pixelColor(p[0]+dx+ox,p[1]+dy+oy).red())for ox in [-1,0,1]for oy in [-1,0,1])for dx,dy in points)
 cross=seen([(6,0),(-6,0),(9,0),(-9,0),(0,6),(0,-6),(0,9),(0,-9)],lambda a,b:a+40<b)
 ring=seen([(3,3),(3,-3),(-3,3),(-3,-3),(4,0),(-4,0),(0,4),(0,-4)],lambda a,b:a>b+5)
 assert cross>=6,('missing Walk crosshair',cross)
 assert ring>=6,('missing Walk ring',ring)
 return dict(cross=cross,ring=ring,samples_each=8)
