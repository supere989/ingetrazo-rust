
struct AnnotationDraw {uint2 gates;uint kind;uint padding;float points[16];};
uint2 product64(int a,int b){return uint2(uint(a)*uint(b),uint(mulhi(a,b)));}
uint2 subtract64(uint2 a,uint2 b){return uint2(a.x-b.x,a.y-b.y-uint(a.x<b.x));}
uint2 edge64(int2 a,int2 b,int2 p){int2 d=b-a,q=p-a;return subtract64(product64(d.x,q.y),product64(d.y,q.x));}
bool positive64(uint2 v){return int(v.y)>0||(v.y==0u&&v.x!=0u);}
bool source_edge(int2 a,int2 b,int2 p){int2 d=b-a;uint2 e=edge64(a,b,p);return positive64(e)||(all(e==uint2(0))&&(d.y<0||(d.y==0&&d.x<0)));}
bool source_triangle(int2 a,int2 b,int2 c,int2 p){uint2 area=edge64(a,b,c);if(all(area==uint2(0)))return false;if(!positive64(area)){int2 t=b;b=c;c=t;}return source_edge(a,b,p)&&source_edge(b,c,p)&&source_edge(c,a,p);}
bool annotation_visible(const device AnnotationDraw& draw,const device uint* hidden,int2 point){
 if((draw.gates.x!=0xffffffffu&&hidden[draw.gates.x])||(draw.gates.y!=0xffffffffu&&hidden[draw.gates.y]))return false;
 if(draw.kind==1u){int2 a=int2(rint(float2(draw.points[0],draw.points[1])*256.)),b=int2(rint(float2(draw.points[2],draw.points[3])*256.)),c=int2(rint(float2(draw.points[4],draw.points[5])*256.));return source_triangle(a,b,c,point*256+int2(128));}
 return true;
}
