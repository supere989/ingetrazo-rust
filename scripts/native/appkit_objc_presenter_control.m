// Diagnostic-only Apple SDK reference. No production fallback or Rust bindings.
#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <MetalKit/MetalKit.h>
#import <QuartzCore/QuartzCore.h>
#include <math.h>

@interface PresentationSignal : NSObject
@property BOOL seen;
@property double time;
@property NSUInteger drawableID;
@end
@implementation PresentationSignal
@end

@interface PresentationFrame : NSObject
@property(nonatomic,strong) id<MTLCommandBuffer> command;
@property(nonatomic,strong) PresentationSignal *signal;
@property(nonatomic,copy) MTLDrawablePresentedHandler handler;
@property(nonatomic) NSUInteger drawableID;
@end
@implementation PresentationFrame
@end

@interface PresenterReference : NSObject<MTKViewDelegate>
@property(nonatomic,strong) NSApplication *app;
@property(nonatomic,strong) NSWindow *window;
@property(nonatomic,strong) MTKView *view;
@property(nonatomic,strong) id<MTLDevice> device;
@property(nonatomic,strong) id<MTLCommandQueue> queue;
@property(nonatomic,strong) NSMutableArray<PresentationFrame *> *frames;
@property(nonatomic,strong) NSMutableArray<NSDictionary *> *terminalProof;
@property(nonatomic,strong) NSTimer *timer;
@property(nonatomic,copy) NSString *path;
@property(nonatomic,copy) NSString *error;
@property(nonatomic) double started;
@property(nonatomic) NSUInteger drawCalls,submitted,displayed,skipped,noDrawable;
@property(nonatomic) BOOL passed,finished;
-(void)poll;
-(void)finish;
@end

@implementation PresenterReference
-(void)poll {
    for (NSUInteger i=0;i<self.frames.count;) {
        PresentationFrame *f=self.frames[i];
        MTLCommandBufferStatus status=f.command.status;
        if(status==MTLCommandBufferStatusError) {
            self.error=[NSString stringWithFormat:@"Command error: %@",f.command.error]; return;
        }
        BOOL seen; double time; NSUInteger callbackID;
        @synchronized(f.signal) { seen=f.signal.seen; time=f.signal.time; callbackID=f.signal.drawableID; }
        if(status==MTLCommandBufferStatusCompleted && seen) {
            if(callbackID!=f.drawableID) {self.error=@"Callback drawable identity mismatch; owners retained";return;}
            if(!isfinite(time)||time<0.) { self.error=@"Invalid presentedTime; owners retained";return; }
            if(time>0.) self.displayed++; else self.skipped++;
            if(self.terminalProof.count>=16) [self.terminalProof removeObjectAtIndex:0];
            [self.terminalProof addObject:@{@"drawable_id":@(f.drawableID),@"callback_drawable_id":@(callbackID),@"command_status":@(status),@"presented_time":@(time),@"gpu_start":@(f.command.GPUStartTime),@"gpu_end":@(f.command.GPUEndTime)}];
            [self.frames removeObjectAtIndex:i];
        } else i++;
    }
}
-(void)drawInMTKView:(MTKView *)view {
    @autoreleasepool {
        self.drawCalls++;
        [self poll];
        if(self.frames.count>=3||self.error||self.finished) return;
        MTLRenderPassDescriptor *pass=view.currentRenderPassDescriptor;
        if(!pass) {self.noDrawable++;return;}
        id<CAMetalDrawable> drawable=view.currentDrawable;
        if(!drawable) {self.error=@"MTKView descriptor lacks drawable";return;}
        pass.colorAttachments[0].loadAction=MTLLoadActionClear;
        pass.colorAttachments[0].storeAction=MTLStoreActionStore;
        pass.colorAttachments[0].clearColor=MTLClearColorMake(.08,.2,.35,1.);
        id<MTLCommandBuffer> command=[self.queue commandBuffer];
        if(!command) {self.error=@"Command allocation failed";return;}
        id<MTLRenderCommandEncoder> encoder=[command renderCommandEncoderWithDescriptor:pass];
        if(!encoder) {self.error=@"Render encoder allocation failed";return;}
        [encoder endEncoding];
        PresentationSignal *signal=[PresentationSignal new];
        PresentationFrame *frame=[PresentationFrame new];
        frame.command=command;frame.signal=signal;frame.drawableID=drawable.drawableID;
        // Literal SDK block parameter/message dispatch; no Rust block/atomic/binding path.
        frame.handler=^(id<MTLDrawable> actualDrawable) {
            double time=actualDrawable.presentedTime;
            @synchronized(signal) {signal.time=time;signal.drawableID=actualDrawable.drawableID;signal.seen=YES;}
        };
        [drawable addPresentedHandler:frame.handler];
        [command presentDrawable:drawable];
        [command commit];
        [self.frames addObject:frame];self.submitted++;
        // ARC releases the local drawable at this pool boundary; command/block owners persist.
    }
}
-(void)mtkView:(MTKView *)view drawableSizeWillChange:(CGSize)size {(void)view;(void)size;}
-(void)tick {
    [self poll];
    if(self.displayed>=2) self.view.paused=YES;
    self.passed=self.displayed>=2&&self.frames.count==0&&!self.error;
    if(self.passed||self.error) [self finish];
    else if(CACurrentMediaTime()-self.started>15.) {
        self.error=@"Objective-C strict positive presentation deadline";[self finish];
    }
}
-(void)finish {
    if(self.finished) return;
    self.finished=YES;self.view.paused=YES;[self.timer invalidate];
    NSMutableArray *frames=[NSMutableArray new];
    for(PresentationFrame *f in self.frames) {
        BOOL seen;double time;NSUInteger callbackID;@synchronized(f.signal){seen=f.signal.seen;time=f.signal.time;callbackID=f.signal.drawableID;}
        [frames addObject:@{@"drawable_id":@(f.drawableID),@"command_status":@(f.command.status),
           @"callback_seen":@(seen),@"callback_drawable_id":@(callbackID),@"presented_time":@(time),
           @"gpu_start":@(f.command.GPUStartTime),@"gpu_end":@(f.command.GPUEndTime)}];
    }
    NSDictionary *report=@{@"passed":@(self.passed),@"error":self.error?:NSNull.null,
       @"scope":@"Literal Apple SDK Objective-C AppKit/MTKView diagnostic; no Rust callback path or production fallback",
       @"device_name":self.device.name?:@"",@"device_registry_id":@(self.device.registryID),
       @"queue_device":@(self.queue.device.registryID),@"layer_device":@(((CAMetalLayer *)self.view.layer).device.registryID),
       @"draw_calls":@(self.drawCalls),@"submitted":@(self.submitted),@"displayed":@(self.displayed),
       @"skipped":@(self.skipped),@"pending":@(self.frames.count),@"no_drawable":@(self.noDrawable),
       @"frames":frames,@"terminal_proof_last16":self.terminalProof,@"application_active":@(self.app.isActive),@"window_visible":@(self.window.isVisible),
       @"window_key":@(self.window.isKeyWindow),@"rust_bindings":@NO,@"winit_hosting":@NO,
       @"framework_timed_updates":@YES,@"framework_current_drawable":@YES};
    NSError *error=nil;NSData *data=[NSJSONSerialization dataWithJSONObject:report options:NSJSONWritingPrettyPrinted error:&error];
    if(!data||![data writeToFile:self.path options:NSDataWritingAtomic error:&error]) {
        self.error=[NSString stringWithFormat:@"Report write failed: %@",error];self.passed=NO;
    }
    if(self.app.isRunning) {
    [self.app stop:nil];
    NSEvent *wake=[NSEvent otherEventWithType:NSEventTypeApplicationDefined location:NSZeroPoint
        modifierFlags:0 timestamp:0 windowNumber:0 context:nil subtype:0 data1:0 data2:0];
    if(wake) [self.app postEvent:wake atStart:YES];
    }
}
@end

// Process-lifetime quarantine retains ALL unresolved frame/queue/view/window/block owners.
static void *unresolvedOwners;
int main(int argc,const char **argv) {
    @autoreleasepool {
        if(argc!=2){fprintf(stderr,"Usage: appkit_objc_presenter_control report.json\n");return 2;}
        PresenterReference *r=[PresenterReference new];r.path=[NSString stringWithUTF8String:argv[1]];
        r.app=NSApplication.sharedApplication;r.app.delegate=nil;
        r.frames=[NSMutableArray new];r.terminalProof=[NSMutableArray new];r.started=CACurrentMediaTime();
        if(![r.app setActivationPolicy:NSApplicationActivationPolicyRegular]) r.error=@"AppKit activation policy refused";
        r.device=MTLCreateSystemDefaultDevice();r.queue=[r.device newCommandQueue];
        if(!r.device||!r.queue) r.error=@"Native device/queue unavailable";
        if(!r.error) {
            r.window=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,800,600)
                styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable backing:NSBackingStoreBuffered defer:NO];
            r.window.releasedWhenClosed=NO;
            r.view=[[MTKView alloc] initWithFrame:NSMakeRect(0,0,800,600) device:r.device];
            if(!r.window||!r.view) r.error=@"Native window/MTKView unavailable";
        }
        if(!r.error) {
            r.view.paused=YES;r.view.enableSetNeedsDisplay=NO;r.view.preferredFramesPerSecond=60;
            r.view.colorPixelFormat=MTLPixelFormatBGRA8Unorm;r.view.framebufferOnly=YES;r.view.delegate=r;
            r.window.contentView=r.view;[r.window center];[r.window makeKeyAndOrderFront:nil];
            [r.app activateIgnoringOtherApps:YES];
            if(r.queue.device.registryID!=r.device.registryID||((CAMetalLayer *)r.view.layer).device.registryID!=r.device.registryID)
                r.error=@"Queue/view native device mismatch";
        }
        if(r.error) [r finish];
        else {
            r.timer=[NSTimer scheduledTimerWithTimeInterval:.016 repeats:YES block:^(NSTimer *timer){(void)timer;@autoreleasepool{[r tick];}}];
            r.view.paused=NO;[r.app run];
            if(!r.finished){r.error=@"NSApplication stopped before proof";[r finish];}
        }
        [r.timer invalidate];r.timer=nil;
        BOOL passed=r.passed&&!r.error;
        if(r.frames.count) {
            fprintf(stderr,"Objective-C control quarantines unresolved complete presenter owners\n");
            unresolvedOwners=(__bridge_retained void *)r;
        }
        return passed?0:1;
    }
}
