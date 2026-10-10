// Independent direct AppKit/CAMetalLayer diagnosis; no product renderer dispatch.
// Same command.present(drawable) + addPresentedHandler API as native Rust host.
import AppKit
import Metal
import QuartzCore
import Foundation

final class Frame {
    let command: MTLCommandBuffer
    let drawable: CAMetalDrawable
    let submission: UInt64
    private let lock = NSLock()
    private var callbackSeen = false
    private var callbackTime: Double = 0
    init(_ command: MTLCommandBuffer, _ drawable: CAMetalDrawable, _ submission: UInt64) {
        self.command = command; self.drawable = drawable; self.submission = submission
    }
    func callback(_ received: MTLDrawable) {
        lock.lock(); callbackTime = received.presentedTime; callbackSeen = true; lock.unlock()
    }
    func observation() -> (Bool, Double) {
        lock.lock(); defer { lock.unlock() }; return (callbackSeen, callbackTime)
    }
}
final class Reference: NSObject, NSApplicationDelegate, NSWindowDelegate {
    let device: MTLDevice
    let queue: MTLCommandQueue
    let layer = CAMetalLayer()
    let window: NSWindow
    let view: NSView
    let output: URL
    var timer: Timer?
    var frames: [Frame] = []
    var submitted: UInt64 = 0
    var retired: UInt64 = 0
    var displayed: UInt64 = 0
    var skipped: UInt64 = 0
    var resized = false
    var resizedBaseline: UInt64 = 0
    var resizeRequested = false
    var exactResizeObserved = false
    var started = Date()
    var observationAt = Date()
    var failure: String?
    var passed = false
    var finished = false
    init(_ output: URL) throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else {
            throw NSError(domain: "MetalReference", code: 1, userInfo: [NSLocalizedDescriptionKey: "No actual native Metal device/queue"])
        }
        self.device = device; self.queue = queue; self.output = output
        self.window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
                               styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        self.view = NSView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        super.init()
        window.title = "IngeTrazo direct AppKit Metal reference"
        window.contentView = view; window.delegate = self
        layer.device = device; layer.pixelFormat = .bgra8Unorm
        layer.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        layer.framebufferOnly = true; layer.maximumDrawableCount = 3
        layer.allowsNextDrawableTimeout = true; layer.displaySyncEnabled = true
        // Custom layer-hosted view: install supplied layer before wantsLayer.
        view.layer = layer; view.wantsLayer = true
        layer.frame = view.bounds
        updateExtent()
    }
    func updateExtent() {
        let density = window.backingScaleFactor
        layer.contentsScale = density; layer.frame = view.bounds
        layer.drawableSize = CGSize(width: view.bounds.width * density, height: view.bounds.height * density)
        if resizeRequested && view.bounds.size == NSSize(width: 640, height: 480) {
            exactResizeObserved = true
        }
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        window.center(); window.makeKeyAndOrderFront(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
        started = Date(); observationAt = started
        print("Direct AppKit device=\(device.name) unified=\(device.hasUnifiedMemory) window=\(window.windowNumber)")
        timer = Timer(timeInterval: 0.016, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(timer!, forMode: .common)
    }
    func windowDidResize(_ notification: Notification) { updateExtent() }
    func windowWillClose(_ notification: Notification) { failure = "Own reference window closed before acceptance"; finish() }
    func poll() -> Bool {
        // Independent bounded scan: later terminal callbacks never release unknown head.
        for index in frames.indices.reversed() {
            let frame = frames[index]
            if frame.command.status == .error { failure = "Actual GPU command error: \(String(describing: frame.command.error))"; return false }
            let (seen, time) = frame.observation()
            if frame.command.status != .completed || !seen { continue }
            if !time.isFinite || time < 0 { failure = "Invalid actual presented callback timestamp"; return false }
            if time > 0 { displayed += 1 } else { skipped += 1 }
            retired += 1; frames.remove(at: index)
        }
        return true
    }
    func tick() {
        if finished { return }
        if !poll() { finish(); return }
        if Date().timeIntervalSince(started) > 15 { failure = "Unchanged15s positive presentation deadline"; finish(); return }
        if Date().timeIntervalSince(observationAt) > 1 {
            print("Direct observation submitted=\(submitted) retired=\(retired) displayed=\(displayed) skipped=\(skipped) pending=\(frames.count) visible=\(window.isVisible)")
            observationAt = Date()
        }
        if displayed >= 2 && frames.isEmpty && !resizeRequested {
            resizeRequested = true; resizedBaseline = displayed
            window.setContentSize(NSSize(width: 640, height: 480)); updateExtent()
            resized = exactResizeObserved
        }
        if resized && exactResizeObserved && displayed >= resizedBaseline + 4 && displayed >= 6 && frames.isEmpty {
            passed = true; finish(); return
        }
        let target = resized ? resizedBaseline + 4 : 2
        if displayed >= target || frames.count >= 3 { return }
        autoreleasepool {
            guard let drawable = layer.nextDrawable(), let command = queue.makeCommandBuffer() else {
                failure = "Actual drawable/command acquisition refused"; finish(); return
            }
            let pass = MTLRenderPassDescriptor()
            pass.colorAttachments[0].texture = drawable.texture
            pass.colorAttachments[0].loadAction = .clear; pass.colorAttachments[0].storeAction = .store
            pass.colorAttachments[0].clearColor = MTLClearColor(red: 0.08, green: 0.2, blue: 0.35, alpha: 1)
            guard let encoder = command.makeRenderCommandEncoder(descriptor: pass) else {
                failure = "Native clear encoder refused"; finish(); return
            }
            encoder.endEncoding()
            let frame = Frame(command, drawable, submitted + 1)
            drawable.addPresentedHandler { [weak frame] received in frame?.callback(received) }
            frames.append(frame)
            command.present(drawable); command.commit(); submitted += 1
        }
    }
    func finish() {
        if finished { return }; finished = true; timer?.invalidate()
        let unknown = frames.map { frame -> [String: Any] in
            let (seen, time) = frame.observation()
            return ["submission": frame.submission, "drawable_id": frame.drawable.drawableID,
                    "command_status": frame.command.status.rawValue, "callback_seen": seen,
                    "callback_time": time, "presented_time": frame.drawable.presentedTime,
                    "gpu_start": frame.command.gpuStartTime, "gpu_end": frame.command.gpuEndTime]
        }
        let result: [String: Any] = ["passed": passed, "device": device.name, "unified": device.hasUnifiedMemory,
            "implementation": "direct AppKit/CAMetalLayer, independent of winit", "submitted": submitted,
            "retired": retired, "displayed": displayed, "skipped": skipped, "pending": frames.count,
            "exact_resize": exactResizeObserved, "resize_baseline": resizedBaseline,
            "elapsed_seconds": Date().timeIntervalSince(started), "unknown_frames": unknown,
            "failure": failure ?? "", "physical_mac_claim": false]
        do {
            try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
            try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: output.appendingPathComponent("result.json"))
        } catch { failure = "Retaining native result failed: \(error)"; passed = false }
        print("Direct AppKit result passed=\(passed) displayed=\(displayed) skipped=\(skipped) pending=\(frames.count)")
        // Keep all owners intact until actual terminal proof. No unknown release on failure.
        if !frames.isEmpty { _ = Unmanaged.passRetained(self); print("Unknown direct AppKit owners retained") }
        NSApplication.shared.stop(nil)
        let wake = NSEvent.otherEvent(with: .applicationDefined, location: .zero, modifierFlags: [], timestamp: 0,
                                     windowNumber: 0, context: nil, subtype: 0, data1: 0, data2: 0)!
        NSApplication.shared.postEvent(wake, atStart: false)
    }
}
let output = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? "appkit-reference")
do {
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    let reference = try Reference(output)
    app.delegate = reference
    app.run()
    exit(reference.passed ? 0 : 1)
} catch { fputs("Direct AppKit refusal: \(error)\n", stderr); exit(1) }
