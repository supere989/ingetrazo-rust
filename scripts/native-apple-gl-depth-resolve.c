/* Native Apple observation; no inferred universal selector.
 * clang -std=c11 -Wno-deprecated-declarations native-apple-gl-depth-resolve.c -framework OpenGL -o witness
 * Run witness > apple-gl-depth-resolve.json. Exit nonzero retains a JSON error.
 */
#include <OpenGL/OpenGL.h>
#include <OpenGL/gl3.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
static void fail(const char *s){printf("{\"error\":\"%s\"}\n",s);exit(1);}
static GLuint shader(GLenum type,const char *src){GLuint s=glCreateShader(type);glShaderSource(s,1,&src,NULL);glCompileShader(s);GLint ok;glGetShaderiv(s,GL_COMPILE_STATUS,&ok);if(!ok)fail("shader compile");return s;}
int main(void){
 CGLPixelFormatAttribute attrs[]={kCGLPFAOpenGLProfile,(CGLPixelFormatAttribute)kCGLOGLPVersion_3_2_Core,kCGLPFAAccelerated,(CGLPixelFormatAttribute)0};
 CGLPixelFormatObj fmt=NULL;GLint n=0;CGLContextObj ctx=NULL;
 if(CGLChoosePixelFormat(attrs,&fmt,&n)!=kCGLNoError||!fmt)fail("accelerated pixel format");
 if(CGLCreateContext(fmt,NULL,&ctx)!=kCGLNoError)fail("context creation");CGLDestroyPixelFormat(fmt);CGLSetCurrentContext(ctx);
 GLuint vs=shader(GL_VERTEX_SHADER,"#version 150\nvoid main(){vec2 p[3]=vec2[3](vec2(-1,-1),vec2(3,-1),vec2(-1,3));gl_Position=vec4(p[gl_VertexID],0,1);}");
 GLuint fs=shader(GL_FRAGMENT_SHADER,"#version 150\nuniform float depth;out vec4 color;void main(){gl_FragDepth=depth;color=vec4(1);}");
 GLuint prog=glCreateProgram();glAttachShader(prog,vs);glAttachShader(prog,fs);glLinkProgram(prog);GLint ok;glGetProgramiv(prog,GL_LINK_STATUS,&ok);if(!ok)fail("program link");glUseProgram(prog);
 GLuint vao;glGenVertexArrays(1,&vao);glBindVertexArray(vao);glViewport(0,0,8,8);glEnable(GL_DEPTH_TEST);glDepthFunc(GL_ALWAYS);glDepthMask(GL_TRUE);glEnable(GL_MULTISAMPLE);glDisable(GL_BLEND);
 GLint renderer=0;CGLGetParameter(ctx,kCGLCPCurrentRendererID,&renderer);
 printf("{\"renderer\":\"%s\",\"vendor\":\"%s\",\"version\":\"%s\",\"cgl_renderer_id\":%d,\"same_metal_device_verified\":false,\"cases\":[",glGetString(GL_RENDERER),glGetString(GL_VENDOR),glGetString(GL_VERSION),renderer);
 for(int count=2;count<=4;count+=2){
 GLuint fbo[2],rb[3];glGenFramebuffers(2,fbo);glGenRenderbuffers(3,rb);glBindFramebuffer(GL_FRAMEBUFFER,fbo[0]);
 glBindRenderbuffer(GL_RENDERBUFFER,rb[0]);glRenderbufferStorageMultisample(GL_RENDERBUFFER,count,GL_RGBA8,8,8);glFramebufferRenderbuffer(GL_FRAMEBUFFER,GL_COLOR_ATTACHMENT0,GL_RENDERBUFFER,rb[0]);
 glBindRenderbuffer(GL_RENDERBUFFER,rb[1]);glRenderbufferStorageMultisample(GL_RENDERBUFFER,count,GL_DEPTH24_STENCIL8,8,8);glFramebufferRenderbuffer(GL_FRAMEBUFFER,GL_DEPTH_STENCIL_ATTACHMENT,GL_RENDERBUFFER,rb[1]);
 GLint actual=0,format=0;glGetRenderbufferParameteriv(GL_RENDERBUFFER,GL_RENDERBUFFER_SAMPLES,&actual);glGetRenderbufferParameteriv(GL_RENDERBUFFER,GL_RENDERBUFFER_INTERNAL_FORMAT,&format);
 if(glCheckFramebufferStatus(GL_FRAMEBUFFER)!=GL_FRAMEBUFFER_COMPLETE)fail("multisample framebuffer");
 printf("%s{\"requested\":%d,\"actual\":%d,\"format\":%d,\"positions_bottom_origin\":[",count==2?"":",",count,actual,format);
 for(int i=0;i<actual;i++){float xy[2];glGetMultisamplefv(GL_SAMPLE_POSITION,i,xy);printf("%s[%.9g,%.9g]",i?",":"",xy[0],xy[1]);}
 glBindFramebuffer(GL_FRAMEBUFFER,fbo[1]);glBindRenderbuffer(GL_RENDERBUFFER,rb[2]);glRenderbufferStorage(GL_RENDERBUFFER,GL_DEPTH24_STENCIL8,8,8);glFramebufferRenderbuffer(GL_FRAMEBUFFER,GL_DEPTH_STENCIL_ATTACHMENT,GL_RENDERBUFFER,rb[2]);glDrawBuffer(GL_NONE);glReadBuffer(GL_NONE);if(glCheckFramebufferStatus(GL_FRAMEBUFFER)!=GL_FRAMEBUFFER_COMPLETE)fail("resolve framebuffer");
 printf("],\"shifts\":[");
 for(int shift=0;shift<actual;shift++){
 glBindFramebuffer(GL_FRAMEBUFFER,fbo[0]);glDisable(GL_SAMPLE_MASK);glClearDepth(1);glClear(GL_DEPTH_BUFFER_BIT);glEnable(GL_SAMPLE_MASK);
 for(int i=0;i<actual;i++){glSampleMaski(0,1u<<i);glUniform1f(glGetUniformLocation(prog,"depth"),(1.f+((i+shift)%actual))/(actual+1));glDrawArrays(GL_TRIANGLES,0,3);}glDisable(GL_SAMPLE_MASK);
 glBindFramebuffer(GL_READ_FRAMEBUFFER,fbo[0]);glBindFramebuffer(GL_DRAW_FRAMEBUFFER,fbo[1]);glBlitFramebuffer(0,0,8,8,0,0,8,8,GL_DEPTH_BUFFER_BIT,GL_NEAREST);glBindFramebuffer(GL_READ_FRAMEBUFFER,fbo[1]);float pixels[64];glReadPixels(0,0,8,8,GL_DEPTH_COMPONENT,GL_FLOAT,pixels);if(glGetError()!=GL_NO_ERROR)fail("GL operation");
 int selected=-1;for(int i=0;i<actual;i++){int match=1;float d=(1.f+((i+shift)%actual))/(actual+1);for(int p=0;p<64;p++)if(fabsf(pixels[p]-d)>2e-7f)match=0;if(match)selected=i;}
 printf("%s{\"shift\":%d,\"selected_sample\":%d,\"pixels\":[",shift?",":"",shift,selected);for(int p=0;p<64;p++)printf("%s%.9g",p?",":"",pixels[p]);printf("]}");
 }printf("]}");glDeleteRenderbuffers(3,rb);glDeleteFramebuffers(2,fbo);
 }printf("]}\n");glDeleteProgram(prog);glDeleteShader(vs);glDeleteShader(fs);glDeleteVertexArrays(1,&vao);CGLSetCurrentContext(NULL);CGLDestroyContext(ctx);return 0;
}
