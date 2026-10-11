pub mod cad2d;
pub mod resize;
pub mod headless;
mod source_depth;
mod source_discard;
pub mod world;
mod annotation_visibility;
pub mod annotation_viewport;
pub mod annotation_overlay_probe;
pub mod hidden_ghost_application_probe;
pub mod face_sweep_application_probe;
mod annotation_visibility_gpu;
pub mod resources;
pub mod texture_cache;
mod queues;
pub mod lifetime;
pub mod budget;
mod annotation_stencil;
mod annotation_timing;
// Adapted from egui-ash-renderer 0.7.0 example (MIT, Adrien Bennadji).


use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk, Device, Entry, Instance,
};
use egui::{ClippedPrimitive, Context, TextureId, ViewportId};
use egui_ash_renderer::{Options, Renderer};
use egui_winit::State;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    error::Error,
    ffi::{CStr, CString},
    os::raw::c_void,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{WindowEvent,DeviceEvent,DeviceId},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

fn retain_frame_texture_deltas(pending:&mut std::collections::VecDeque<(egui::TextureId,egui::epaint::ImageDelta)>,free:&mut Option<Vec<egui::TextureId>>,set:Vec<(egui::TextureId,egui::epaint::ImageDelta)>,releases:&[egui::TextureId]){
    pending.extend(set);
    if !releases.is_empty(){free.get_or_insert_with(Vec::new).extend_from_slice(releases);}
}
/// UI-order slot resolved only after the current frame's GPU transaction succeeds.
struct AnnotationSlot;
fn annotation_slot()->egui::Shape{egui::Shape::Callback(egui::PaintCallback{rect:egui::Rect::NOTHING,callback:std::sync::Arc::new(AnnotationSlot)})}
fn resolve_annotation_slots(shapes:&mut [egui::epaint::ClippedShape],ready:Option<&egui::Shape>){
    fn resolve(shape:&mut egui::Shape,ready:Option<&egui::Shape>){
        match shape {
            egui::Shape::Callback(slot) if slot.callback.is::<AnnotationSlot>()=>{
                let rect=slot.rect;
                *shape=match ready{Some(egui::Shape::Callback(callback))=>{let mut callback=callback.clone();callback.rect=rect;egui::Shape::Callback(callback)},_=>egui::Shape::Noop};
            },
            egui::Shape::Vec(children)=>for child in children{resolve(child,ready)},
            _=>{},
        }
    }
    for shape in shapes{resolve(&mut shape.shape,ready)}
}

pub trait App {
    fn title() -> &'static str;
    fn new(app: &mut System, initial_file: Option<std::path::PathBuf>) -> Self;
    fn build_ui(&mut self, ctx: &Context);
    fn prepare_gpu(&mut self,_:&mut System,_resize:bool) {}
    fn gpu_error(&mut self,error:&str) {eprintln!("GPU rendering paused: {error}");}
    fn clean(&mut self, vulkan_ctx: &VulkanContext) {
        let _ = vulkan_ctx;
    }
}

pub fn run_app<A: App>(initial_file: Option<std::path::PathBuf>) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<()>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app: AppWrapper<A> = AppWrapper {
        title: A::title(),
        proxy: event_loop.create_proxy(),
        window: None,
        app: None,
        egui_app: None,
        initial_file,
        initialization_error:None,
    };

    event_loop.run_app(&mut app)?;
    if let Some(error)=app.initialization_error{return Err(error.into());}
    Ok(())
}

struct AppWrapper<A: App> {
    title: &'static str,
    proxy: winit::event_loop::EventLoopProxy<()>,
    window: Option<Window>,
    app: Option<System>,
    egui_app: Option<A>,
    initial_file: Option<std::path::PathBuf>,
    initialization_error:Option<String>,
}

impl<A: App> ApplicationHandler for AppWrapper<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = match create_window(self.title,event_loop){Ok(window)=>window,Err(error)=>{self.initialization_error=Some(error.to_string());event_loop.exit();return;}};
        let mut app = match System::new(self.title,&window){Ok(system)=>system,Err(error)=>{self.initialization_error=Some(error.to_string());event_loop.exit();return;}};
        let proxy=self.proxy.clone();
        app.egui_ctx.set_request_repaint_callback(move |info| { if info.delay.is_zero() { let _=proxy.send_event(()); } });
        let egui_app = A::new(&mut app, self.initial_file.take());

        self.window = Some(window);
        self.app = Some(app);
        self.egui_app = Some(egui_app)
    }

    fn user_event(&mut self,_:&ActiveEventLoop,_:()) {
        if let (Some(app),Some(window))=(&mut self.app,&self.window) { app.repaint_at=std::time::Instant::now();window.request_redraw(); }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let (Some(app),Some(window))=(&self.app,&self.window) {
            if !app.run { event_loop.exit(); return; }
            if std::time::Instant::now()>=app.repaint_at { window.request_redraw(); }
            event_loop.set_control_flow(ControlFlow::WaitUntil(app.repaint_at));
        }
    }
    fn window_event(&mut self,event_loop:&ActiveEventLoop,_:WindowId,event:WindowEvent) {
        if let (Some(app),Some(window),Some(egui_app))=(&mut self.app,&self.window,&mut self.egui_app) {
            if matches!(event,WindowEvent::RedrawRequested) { app.draw(egui_app,window); }
            else { app.on_window_event(&event,window,event_loop); }
        }
    }

    fn device_event(&mut self,_:&ActiveEventLoop,_:DeviceId,event:DeviceEvent){
        if let DeviceEvent::MouseMotion{delta}=event {
            if let (Some(app),Some(window))=(&mut self.app,&self.window){
                if window.has_focus(){app.egui_winit.on_mouse_motion(delta);window.request_redraw();}
            }
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        // Startup may have failed before either owner was published. The
        // unsubmitted guards already released those candidates; only a live
        // application has submitted resources requiring the normal exit drain.
        if let (Some(system), Some(app)) = (&mut self.app, &mut self.egui_app) {
            system.on_exit(app);
        }
    }
}

pub struct System {
    command_buffer: vk::CommandBuffer,
    swapchain: Swapchain,
    image_available_semaphore: vk::Semaphore,
    // Presentation may outlive the graphics fence; one semaphore per acquired image.
    render_finished_semaphores: Vec<vk::Semaphore>,
    present_fences:Vec<PresentFence>,
    presented_without_fence:bool,
    exited:bool,
    fence: vk::Fence,

    run: bool,
    dirty_swapchain: bool,

    pub egui_ctx: Context,
    pub egui_winit: State,
    pointer_modifiers:crate::pointer_modifiers::Handle,
    pub renderer: std::mem::ManuallyDrop<Renderer>,
    pub memory_budget: std::sync::Arc<budget::VulkanBudget>,
    pub world: Option<world::World>,
    pub cad2d: Option<cad2d::Renderer>,
    annotation_cad:Option<cad2d::Renderer>,
    annotation_viewport:Option<annotation_viewport::ViewportAnnotations>,
    annotation_prepare_error:Option<String>,
    annotation_font_refusal_for_probe:bool,
    annotation_ready_shape:Option<egui::Shape>,
    annotation_timing:Option<annotation_timing::Timing>,
    annotation_sample:Option<u32>,
    draw_serial:u64,
    completed_present_serial_for_probe:u64,

    textures_to_free: Option<Vec<TextureId>>,
    close_requested: bool,
    repaint_at: std::time::Instant,
    pending_texture_updates: std::collections::VecDeque<(TextureId,egui::epaint::ImageDelta)>,
    last_gpu_error: Option<String>,

    pub vulkan_context: VulkanContext,
}

impl System {
    pub fn new(title: &str, window: &Window) -> Result<Self, Box<dyn Error>> {
        Self::new_controlled(title,window,&resize::Control::default())
    }
    /// Cold unsubmitted startup control; successful actual owners must unwind.
    pub fn verify_startup_cancellation_for_probe(title:&str,window:&Window,stage:resize::Stage)->Result<resize::Audit,String>{
        let audit=std::sync::Arc::new(std::sync::Mutex::new(resize::Audit::default()));
        match Self::new_controlled(title,window,&resize::Control{cancel:Some(stage),audit:Some(audit.clone())}){
            Err(error) if error.to_string().starts_with("Engineering resize cancellation")=>{},
            Err(error)=>return Err(error.to_string()),Ok(_)=>return Err("Expected startup cancellation".into()),
        }
        let observed=audit.lock().map_err(|_|"Audit lock poisoned")?.clone();
        if observed.created!=observed.destroyed{return Err(format!("Unsubmitted startup owner leak: {observed:?}"));}
        Ok(observed)
    }
    /// Observe a real strict-adapter refusal (no fabricated driver return).
    pub fn verify_startup_refusal_for_probe(title:&str,window:&Window)->Result<resize::Audit,String>{
        let audit=std::sync::Arc::new(std::sync::Mutex::new(resize::Audit::default()));
        let error=match Self::new_controlled(title,window,&resize::Control{cancel:None,audit:Some(audit.clone())}){Err(error)=>error,Ok(_)=>return Err("Expected unmatched adapter override".into())};
        if !error.to_string().starts_with("No suitable Vulkan adapter"){return Err(error.to_string());}
        let observed=audit.lock().map_err(|_|"Audit lock poisoned")?.clone();
        if observed.created!=observed.destroyed||observed.created.get(&resize::Kind::Surface)!=Some(&1)||observed.created.contains_key(&resize::Kind::Device){return Err(format!("Adapter refusal leaked unpublished context: {observed:?}"));}
        Ok(observed)
    }
    fn new_controlled(title:&str,window:&Window,control:&resize::Control)->Result<Self,Box<dyn Error>>{
        let annotation_sample=match std::env::var("INGETRAZO_ANNOTATION_DEPTH_SAMPLE"){
            Ok(value)=>Some(value.parse::<u32>().map_err(|_|std::io::Error::new(std::io::ErrorKind::InvalidInput,"Invalid annotation depth sample policy"))?),
            Err(std::env::VarError::NotPresent)=>None,Err(error)=>return Err(error.into()),
        };

        log::info!("Create application");

        let vulkan_context = VulkanContext::new_controlled(&window, title,control)?;

        let mut startup=UnpublishedStartup{device:vulkan_context.device.clone(),pool:vulkan_context.command_pool,command:None,acquire:None,fence:None,control:control.clone()};
        let command_buffer = {
            let allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(vulkan_context.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            unsafe {
                vulkan_context
                    .device
                    .allocate_command_buffers(&allocate_info)?[0]
            }
        };

        startup.command=Some(command_buffer);control.created(resize::Kind::CommandBuffer,1);control.checkpoint(resize::Stage::StartupCommand)?;
        let memory_budget=std::sync::Arc::new(budget::VulkanBudget::new(&vulkan_context.instance,vulkan_context.physical_device)?);
        memory_budget.bind_lifetime(vulkan_context.lifetime.clone());
        let mut retired=false;
        // Native World includes source-exact floating tool labels. Select the
        // required attachment/pipeline once at startup, retaining the existing
        // explicit unsupported-usage refusal and resize selection.
        let integer=std::env::var_os("INGETRAZO_CAD2D_INTEGER_BLEND").is_some()
            ||std::env::var_os("INGETRAZO_VULKAN_WORLD").is_some();
        let mut pending_swapchain=Swapchain::build(&vulkan_context,memory_budget.clone(),annotation_stencil::selected()?,integer,vk::SwapchainKHR::null(),&mut retired,None,control)?;
        control.checkpoint(resize::Stage::StartupSwapchain)?;
        let swapchain=pending_swapchain.value.as_ref().unwrap();

        // Semaphore use for presentation
        let image_available_semaphore = {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            unsafe {
                vulkan_context
                    .device
                    .create_semaphore(&semaphore_info, None)?
            }
        };
        startup.acquire=Some(image_available_semaphore);control.created(resize::Kind::AcquireSemaphore,1);
        let mut pending_sync=UnpublishedSync{device:vulkan_context.device.clone(),semaphores:create_present_semaphores(&vulkan_context.device,swapchain.images.len())?,present_fences:Vec::new(),control:control.clone()};
        control.created(resize::Kind::Semaphore,pending_sync.semaphores.len()as u64);
        pending_sync.present_fences=create_present_fences(&vulkan_context,swapchain.images.len())?;
        control.created(resize::Kind::PresentFence,pending_sync.present_fences.len()as u64);
        let fence = {
            let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            unsafe { vulkan_context.device.create_fence(&fence_info, None)? }
        };

        startup.fence=Some(fence);control.created(resize::Kind::GraphicsFence,1);control.checkpoint(resize::Stage::StartupSync)?;
        let egui_ctx = Context::default();
        crate::paper_raw_texture::install(&egui_ctx);

        let egui_winit = State::new(
            egui_ctx.clone(),
            ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        );


        let renderer = Renderer::with_memory_observer(
            &vulkan_context.instance,
            vulkan_context.physical_device,
            vulkan_context.device.clone(),
            swapchain.render_pass,
            Options {
                srgb_framebuffer: swapchain.srgb,
                ..Default::default()
            },
            memory_budget.clone(),
        )?;

        let mut renderer=resize::Tracked::new(renderer,control,vec![(resize::Kind::EguiPipeline,1)]);
        control.checkpoint(resize::Stage::StartupRenderer)?;
        // Nothing has been submitted. Move every owner only after all constructors succeed.
        let annotation_timing=annotation_timing::Timing::new(&vulkan_context)?;
        startup.command=None;startup.acquire=None;startup.fence=None;
        let pointer_modifiers=crate::pointer_modifiers::install(&egui_ctx);
        Ok(Self {
            vulkan_context,
            command_buffer,
            swapchain:pending_swapchain.take(),
            image_available_semaphore,
            render_finished_semaphores:std::mem::take(&mut pending_sync.semaphores),
            present_fences:std::mem::take(&mut pending_sync.present_fences),presented_without_fence:false,exited:false,
            fence,

            run: true,
            dirty_swapchain: false,

            egui_ctx,
            egui_winit,
            pointer_modifiers,
            renderer:std::mem::ManuallyDrop::new(renderer.take()),
            memory_budget,
            annotation_timing,world:None,cad2d:None,annotation_cad:None,annotation_viewport:None,annotation_prepare_error:None,annotation_font_refusal_for_probe:false,annotation_ready_shape:None,annotation_sample,draw_serial:0,completed_present_serial_for_probe:0,
            textures_to_free: None,
            close_requested: false,
            repaint_at: std::time::Instant::now(),
            pending_texture_updates:std::collections::VecDeque::new(),
            last_gpu_error:None,
        })
    }

    pub fn bounded_completion_for_probe(&self)->bool{self.wait_gpu_completion()}

    // A released failed set must not be retried if this frame does not paint it.
    // Callback texture ownership is opaque: defer those frames to post-fence retirement.
    fn cancel_unpainted_releases(&mut self,freed:&[egui::TextureId],paint:&[egui::ClippedPrimitive])->Result<(),String>{
        if freed.is_empty()||paint.iter().any(|p|matches!(p.primitive,egui::epaint::Primitive::Callback(_))){return Ok(());}
        let mut dead=std::collections::HashSet::new();dead.try_reserve(freed.len()).map_err(|_|"Released texture identity admission failed")?;dead.extend(freed.iter().copied());
        for p in paint{if let egui::epaint::Primitive::Mesh(mesh)=&p.primitive{dead.remove(&mesh.texture_id);}}
        let mut ids=Vec::new();ids.try_reserve(dead.len()).map_err(|_|"Released texture list admission failed")?;ids.extend(dead);
        crate::paper_raw_texture::cancel(&self.egui_ctx,&ids,&mut self.pending_texture_updates)
    }

    fn upload_pending_textures(&mut self)->Result<(),String>{
        while let Some(delta)=self.pending_texture_updates.front() {
            let raw=crate::paper_raw_texture::peek(&self.egui_ctx,delta.0)?;
            let encoded=raw.as_ref().map(|image|(delta.0,egui_ash_renderer::EncodedRgbaImage{size:image.size,pixels:image.pixels.as_slice()}));
            let encoded=encoded.as_ref().map(std::slice::from_ref).unwrap_or(&[]);
            self.renderer.set_textures_with_encoded_images(self.vulkan_context.graphics_queue,self.vulkan_context.command_pool,std::slice::from_ref(delta),encoded).map_err(|e|e.to_string())?;
            crate::paper_raw_texture::acknowledge(&self.egui_ctx,delta.0)?;
            self.pending_texture_updates.pop_front();
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn create_raw_texture_for_probe(&self)->Result<egui::TextureHandle,String>{crate::paper_raw_texture::create(&self.egui_ctx,[2,2],vec![8,9,12,68].repeat(4))}
    #[doc(hidden)]
    pub fn managed_texture_for_probe(&self,id:egui::TextureId)->bool{self.renderer.managed_texture_ids_for_probe().contains(&id)}

    /// Actual managed Vulkan allocation/retry/retirement admission probe. No model mutation.
    #[doc(hidden)]
    pub fn verify_raw_texture_lifetime_for_probe(&mut self)->Result<(),String>{
        if !self.pending_texture_updates.is_empty(){return Err("Probe requires drained upload queue".into());}
        self.vulkan_context.wait_transfer(self.fence).map_err(|e|e.to_string())?;
        let ids=self.renderer.managed_texture_ids_for_probe();
        let committed=self.memory_budget.ledger.lock().unwrap().total_committed();
        let serial=self.draw_serial;
        let ordinary=egui::TextureId::Managed(u64::MAX-2);
        let delta=egui::epaint::ImageDelta::full(egui::ColorImage::new([2,2],egui::Color32::RED),egui::TextureOptions::NEAREST);
        self.renderer.set_textures(self.vulkan_context.graphics_queue,self.vulkan_context.command_pool,&[(ordinary,delta)]).map_err(|e|e.to_string())?;
        let retained=self.memory_budget.ledger.lock().unwrap().total_committed();
        if retained<=committed{return Err("Actual image allocation was not charged".into());}
        // Real image allocation admitted, subsequent staging allocation denied.
        let info=vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).extent(vk::Extent3D{width:2,height:2,depth:1}).mip_levels(1).array_layers(1).format(vk::Format::R8G8B8A8_SRGB).tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::TRANSFER_DST|vk::ImageUsageFlags::SAMPLED).samples(vk::SampleCountFlags::TYPE_1);
        let image=unsafe{self.vulkan_context.device.create_image(&info,None)}.map_err(|e|e.to_string())?;
        let size=unsafe{self.vulkan_context.device.get_image_memory_requirements(image)}.size;
        unsafe{self.vulkan_context.device.destroy_image(image,None)};
        for release in [true,false]{
            let handle=crate::paper_raw_texture::create(&self.egui_ctx,[2,2],vec![8,9,12,68].repeat(4))?;
            let id=handle.id();
            let delta=self.egui_ctx.tex_manager().write().take_delta();
            if !delta.free.is_empty(){return Err("Unexpected pending free in probe".into());}
            self.pending_texture_updates.extend(delta.set);
            let limit={let mut l=self.memory_budget.ledger.lock().unwrap();let old=l.total_limit;l.total_limit=Some(retained+size);old};
            let refusal=self.upload_pending_textures();
            self.memory_budget.ledger.lock().unwrap().total_limit=limit;
            if refusal.as_ref().err().is_none_or(|error|!error.contains("GPU allocation 16 exceeds global application budget")){return Err(format!("Expected actual 16-byte staging budget denial, got {refusal:?}"));}
            let l=self.memory_budget.ledger.lock().unwrap();
            if l.total_committed()!=retained||l.heaps.iter().any(|h|h.reserved!=0){return Err("Refused texture leaked GPU admission".into());}drop(l);
            if self.draw_serial!=serial||!self.renderer.managed_texture_ids_for_probe().contains(&ordinary)||self.renderer.managed_texture_ids_for_probe().contains(&id){return Err("Refusal changed retained frame texture ownership".into());}
            if crate::paper_raw_texture::peek(&self.egui_ctx,id)?.is_none(){return Err("Refusal lost retry bytes".into());}
            eprintln!("RAW_LIFETIME refusal release={release} error={:?} committed={retained} frame={serial}",refusal.unwrap_err());
            if release{
                drop(handle);let freed=self.egui_ctx.tex_manager().write().take_delta().free;
                self.cancel_unpainted_releases(&freed,&[])?;
                self.renderer.free_textures(&freed).map_err(|e|e.to_string())?;
                self.upload_pending_textures()?;
                if self.renderer.managed_texture_ids_for_probe().contains(&id)||crate::paper_raw_texture::peek(&self.egui_ctx,id)?.is_some(){return Err("Released refusal uploaded orphan".into());}
            }else{
                self.upload_pending_textures()?;
                if !self.renderer.managed_texture_ids_for_probe().contains(&id)||crate::paper_raw_texture::peek(&self.egui_ctx,id)?.is_some(){return Err("Actual raw retry/ACK failed".into());}
                drop(handle);let freed=self.egui_ctx.tex_manager().write().take_delta().free;
                self.vulkan_context.wait_transfer(self.fence).map_err(|e|e.to_string())?;
                crate::paper_raw_texture::cancel(&self.egui_ctx,&freed,&mut self.pending_texture_updates)?;
                self.renderer.free_textures(&freed).map_err(|e|e.to_string())?;
            }
            if self.memory_budget.ledger.lock().unwrap().total_committed()!=retained{return Err("Raw free did not return ledger baseline".into());}
        }
        // Ordinary managed image update still uses the existing conversion path.
        let patch=egui::epaint::ImageDelta::partial([0,0],egui::ColorImage::new([1,1],egui::Color32::GREEN),egui::TextureOptions::NEAREST);
        self.renderer.set_textures(self.vulkan_context.graphics_queue,self.vulkan_context.command_pool,&[(ordinary,patch)]).map_err(|e|e.to_string())?;
        self.renderer.free_textures(&[ordinary]).map_err(|e|e.to_string())?;
        let final_ids=self.renderer.managed_texture_ids_for_probe();let l=self.memory_budget.ledger.lock().unwrap();
        if l.total_committed()!=committed||l.heaps.iter().any(|h|h.reserved!=0)||final_ids.len()!=ids.len()||ids.iter().any(|id|!final_ids.contains(id)){return Err("Final managed GPU ownership differs from baseline".into());}
        eprintln!("PASS RAW_LIFETIME actual image+staging refusal / released-no-orphan / held-retry-ACK / raw-free / ordinary-partial-update-free; frames={serial} baseline_bytes={committed} final_bytes={} managed={}",l.total_committed(),ids.len());
        Ok(())
    }

    /// Read-only engineering observation; adds no waits or production uploads.
    pub fn face_sweep_observation_for_probe(&self)->serde_json::Value{
        let world=self.world.as_ref().map(|world|{let (uploads,bytes,texture)=world.face_sweep_stats_for_probe();serde_json::json!({"extent":world.target_extent_for_probe(),"image_owners":world.face_sweep_image_stats_for_probe(),"base_uploads":world.uploads,"preview_uploads":uploads,"preview_bytes":bytes,"texture_builds":texture.builds,"texture_uploads":texture.uploads,"sampled_entries":texture.sampled_entries})});
        serde_json::json!({"submitted_serial":self.draw_serial,"completed_present_serial":self.completed_present_serial_for_probe,"prepare_error":self.annotation_prepare_error,"committed_bytes":self.memory_budget.ledger.lock().unwrap().total_committed(),"world":world})
    }
    pub fn hidden_ghost_observation_for_probe(&self)->serde_json::Value{serde_json::json!({"submitted_serial":self.draw_serial,"completed_present_serial":self.completed_present_serial_for_probe,"prepare_error":self.annotation_prepare_error,"world":self.world.as_ref().map(|w|w.hidden_ghost_observation_for_probe())})}
    pub fn billboard_observation_for_probe(&self)->serde_json::Value{serde_json::json!({"submitted_serial":self.draw_serial,"completed_present_serial":self.completed_present_serial_for_probe,"prepare_error":self.annotation_prepare_error,"world":self.world.as_ref().map(|w|w.billboard_stats_for_probe())})}
    pub fn solar_stats_for_probe(&self)->Option<(u64,u64,bool)>{self.world.as_ref().map(|world|world.shadow_stats_for_probe())}
    pub fn submitted_frames_for_probe(&self)->u64{self.draw_serial}

    /// Engineering observation only: never reset/destroy these borrowed handles.
    pub fn pending_present_fences_for_probe(&self)->Vec<vk::Fence>{
        self.present_fences.iter().filter(|p|p.used).map(|p|p.fence).collect()
    }

    pub fn enable_cad2d(&mut self)->Result<&cad2d::Renderer,String>{
        if self.cad2d.is_none(){self.cad2d=Some(cad2d::Renderer::new(&self.vulkan_context,self.memory_budget.clone(),self.swapchain.render_pass,&self.swapchain.image_views,self.swapchain.integer_cad)?);}
        Ok(self.cad2d.as_ref().unwrap())
    }

    /// Annotation callbacks use the existing source-ordered feedback pass.
    /// Capability refusal precedes any renderer allocation.
    pub(crate) fn annotation_factory(&mut self)->Result<cad2d::Factory,String>{
        if !self.swapchain.integer_cad{return Err("Native annotations require admitted integer CAD input-attachment pass".into());}
        if self.annotation_cad.is_none(){self.annotation_cad=Some(cad2d::Renderer::new_annotations(&self.vulkan_context,self.memory_budget.clone(),self.swapchain.render_pass,&self.swapchain.image_views,self.swapchain.stencil_enabled)?);}
        Ok(self.annotation_cad.as_ref().unwrap().factory())
    }
    pub(crate) fn prepare_annotation_overlay(&mut self,handle:&cad2d::Handle,
        queries:std::sync::Arc<[annotation_visibility::Query]>,inverse:[f32;16],eye:[f32;3],forward:[f32;3],sample:u32)->Result<(),String>{
        let renderer=self.annotation_cad.as_mut().ok_or("Annotation renderer absent")?;
        self.world.as_mut().ok_or("Annotation World absent")?.prepare_annotation_overlay(renderer,handle,&self.vulkan_context,queries,inverse,eye,forward,sample,self.fence,self.draw_serial,self.draw_serial+1,self.egui_ctx.pixels_per_point())
    }
    pub(crate) fn clear_annotation_overlay(&mut self)->Result<(),String>{
        if let Some(renderer)=&mut self.annotation_cad{
            self.world.as_mut().ok_or("Annotation World absent")?.clear_annotation_overlay(renderer,&self.vulkan_context,self.fence,self.draw_serial)?;
        }
        Ok(())
    }
    pub fn verify_outline_budget_for_probe(&mut self)->Result<serde_json::Value,String>{if !self.wait_gpu_completion(){return Err("Outline budget probe requires acknowledged GPU completion".into());}self.cad2d.as_mut().ok_or("CAD renderer absent")?.verify_outline_budget_for_probe(&self.vulkan_context,self.fence,self.draw_serial)}
    pub fn verify_outline_candidate_rollback_for_probe(&mut self,handle:&cad2d::arena::Handle)->Result<(),String>{if !self.wait_gpu_completion(){return Err("Outline rollback probe requires acknowledged GPU completion".into());}self.cad2d.as_mut().ok_or("CAD renderer absent")?.verify_outline_candidate_rollback_for_probe(&self.vulkan_context,self.fence,self.draw_serial,handle)}
    pub fn verify_cad2d_rollback_for_probe(&mut self)->Result<(),String>{
        self.enable_cad2d()?;
        self.cad2d.as_mut().unwrap().verify_pressure_rollback_for_probe(&self.vulkan_context,self.fence,self.draw_serial)
    }

    pub fn draw<A: App>(&mut self, egui_app: &mut A, window: &Window) {
        // A poisoned device is never retried from repeated repaint events.
        if self.vulkan_context.lifetime.uncertain(){return;}
        // If swapchain must be recreated wait for windows to not be minimized anymore
        if self.dirty_swapchain {
            let PhysicalSize { width, height } = window.inner_size();
            if width > 0 && height > 0 {
                if let Err(error)=self.resize_transaction(){
                    egui_app.gpu_error(&error);
                    self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);
                    return;
                }

            } else {
                return;
            }
        }

        if !self.run {
            return;
        }

        if let Err(error)=self.vulkan_context.wait_transfer(self.fence){
            egui_app.gpu_error(&format!("GPU draw completion unknown: {error}; resources quarantined"));
            self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);
            return;
        }

        if let Some(timing)=&mut self.annotation_timing{if let Err(error)=timing.complete(){egui_app.gpu_error(&error);return;}}

        // Free last frames textures after the previous frame is done rendering
        if let Some(textures) = self.textures_to_free.take() {
            if let Err(error)=crate::paper_raw_texture::cancel(&self.egui_ctx,&textures,&mut self.pending_texture_updates){egui_app.gpu_error(&error);self.textures_to_free=Some(textures);return;}
            self.renderer
                .free_textures(&textures)
                .expect("Failed to free textures");
        }

        egui_app.prepare_gpu(self,true);
        if self.vulkan_context.lifetime.uncertain(){return;}

        // Generate UI
        let mut raw_input = self.egui_winit.take_egui_input(&window);
        if let Ok(mut timeline)=self.pointer_modifiers.lock(){timeline.begin_frame();}
        if self.close_requested { raw_input.viewports.entry(ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close); }

        let egui::FullOutput {
            platform_output,
            textures_delta,
            mut shapes,
            pixels_per_point,
            viewport_output,
        } = self.egui_ctx.run(raw_input, |ctx| {
            egui_app.build_ui(ctx);
        });

        // ctx.run has transferred texture ownership to this host. Retain it even
        // when the subsequent World/annotation transaction refuses this frame.
        retain_frame_texture_deltas(&mut self.pending_texture_updates,&mut self.textures_to_free,textures_delta.set,&textures_delta.free);
        egui_app.prepare_gpu(self,false);
        self.egui_winit.handle_platform_output(&window, platform_output);
        if let Some(output)=viewport_output.get(&ViewportId::ROOT) {
            let canceled=output.commands.iter().any(|c|matches!(c,egui::ViewportCommand::CancelClose));
            if canceled { self.close_requested=false; }
            if output.commands.iter().any(|c|matches!(c,egui::ViewportCommand::Close)) || (self.close_requested&&!canceled) { self.run=false; }
            let mut info=egui::ViewportInfo::default();
            let mut actions=egui::ahash::HashSet::default();
            egui_winit::process_viewport_commands(&self.egui_ctx,&mut info,output.commands.clone(),window,&mut actions);
            self.repaint_at=std::time::Instant::now().checked_add(output.repaint_delay).unwrap_or_else(||std::time::Instant::now()+std::time::Duration::from_secs(3600));
        }


        if self.annotation_prepare_error.is_some(){self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);return;}
        if self.vulkan_context.lifetime.uncertain(){return;}
        resolve_annotation_slots(&mut shapes,self.annotation_ready_shape.as_ref());
        let clipped_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);
        if let Err(error)=self.cancel_unpainted_releases(&textures_delta.free,&clipped_primitives){egui_app.gpu_error(&error);self.last_gpu_error=Some(error);return;}
        // Current-frame set/use/free is legal: retire only at the next acknowledged
        // post-frame boundary, never discard a texture before its queued paint.
        if let Err(error)=self.upload_pending_textures(){
            if self.last_gpu_error.as_ref()!=Some(&error){egui_app.gpu_error(&error);self.last_gpu_error=Some(error);}
            self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);
            return; // Retain last presented frame and retry pending deltas after pressure falls.
        }
        self.last_gpu_error=None;

        if let Some(cad)=&mut self.cad2d {
            if let Err(error)=cad.prepare(&self.vulkan_context,&clipped_primitives,self.fence,self.draw_serial,self.draw_serial+1,pixels_per_point){
                egui_app.gpu_error(&error);
                self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);
                // Old ready paint remains valid; continue presentation under pressure.
            }
        }

        // Drawing the frame
        let next_image_result = unsafe {
            self.swapchain.loader.acquire_next_image(
                self.swapchain.khr,
                lifetime::WAIT_NS,
                self.image_available_semaphore,
                vk::Fence::null(),
            )
        };
        let image_index = match next_image_result {
            Ok((image_index, _)) => image_index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.dirty_swapchain = true;
                return;
            }
            Err(vk::Result::TIMEOUT)|Err(vk::Result::NOT_READY)=>{egui_app.gpu_error("Swapchain image acquisition timed out; frame deferred");self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);return;},
            Err(error)=>{self.vulkan_context.lifetime.quarantine();egui_app.gpu_error(&format!("Image acquisition failed: {error}; resources quarantined"));return;},
        };

        if let Some(cad)=&mut self.cad2d{
            if let Err(error)=cad.set_input_image(image_index as usize){
                self.vulkan_context.lifetime.quarantine();
                egui_app.gpu_error(&format!("CAD input selection failed after image acquisition: {error}; resources quarantined"));return;
            }
        }
        if let Some(cad)=&mut self.annotation_cad{if let Err(error)=cad.set_input_image(image_index as usize){self.vulkan_context.lifetime.quarantine();egui_app.gpu_error(&error);return;}}
        if let Some(present)=self.present_fences.get_mut(image_index as usize){
            if present.used{if let Err(error)=self.vulkan_context.lifetime.wait(present.fence,lifetime::WAIT_NS){egui_app.gpu_error(&format!("Previous presentation completion unknown: {error}"));return;}self.completed_present_serial_for_probe=self.completed_present_serial_for_probe.max(present.serial);present.used=false;}
            if let Err(error)=unsafe{self.vulkan_context.device.reset_fences(&[present.fence])}{self.vulkan_context.lifetime.quarantine();egui_app.gpu_error(&error.to_string());return;}
        }
        if let Err(error)=unsafe{self.vulkan_context.device.reset_fences(&[self.fence])}{self.vulkan_context.lifetime.quarantine();egui_app.gpu_error(&error.to_string());return;}

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT|if self.swapchain.integer_cad{vk::PipelineStageFlags::FRAGMENT_SHADER}else{vk::PipelineStageFlags::empty()}];
        let wait_semaphores = [self.image_available_semaphore];
        let signal_semaphores = [self.render_finished_semaphores[image_index as usize]];

        // Re-record commands to draw geometry
        let draw_error=match record_command_buffers(
            &self.vulkan_context.device,
            self.vulkan_context.command_pool,
            self.command_buffer,
            self.swapchain.framebuffers[image_index as usize],
            self.swapchain.render_pass,
            self.swapchain.extent,
            pixels_per_point,
            &mut self.renderer,
            &clipped_primitives,
            self.world.as_mut(),self.draw_serial+1,self.annotation_timing.as_ref().map(|t|t.pool),
        ){Ok(result)=>result,Err(error)=>{self.vulkan_context.lifetime.quarantine();egui_app.gpu_error(&error.to_string());return;}};
        if let Some(error)=draw_error {egui_app.gpu_error(&error);self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);}

        let command_buffers = [self.command_buffer];
        let submit_info = [vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)];
        if let Err(error)=unsafe {self.vulkan_context.device.queue_submit(self.vulkan_context.graphics_queue,&submit_info,self.fence)}{
            self.vulkan_context.lifetime.quarantine();egui_app.gpu_error(&format!("GPU submission failed: {error}; resources quarantined"));return;
        }

        if let Some(timing)=&mut self.annotation_timing{timing.submitted();}
        self.draw_serial+=1;
        let swapchains = [self.swapchain.khr];
        let images_indices = [image_index];
        let present_handles=self.present_fences.get(image_index as usize).map(|p|vec![p.fence]).unwrap_or_default();
        let mut present_fence_info=vk::SwapchainPresentFenceInfoEXT::default().fences(&present_handles);
        let mut present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&images_indices);

        if !present_handles.is_empty(){present_info=present_info.push_next(&mut present_fence_info);}
        if let Some(p)=self.present_fences.get_mut(image_index as usize){p.used=true;p.serial=self.draw_serial;}else{self.presented_without_fence=true;}
        let present_result = unsafe {
            self.swapchain
                .loader
                .queue_present(self.vulkan_context.present_queue, &present_info)
        };
        match present_result {
            Ok(is_suboptimal) if is_suboptimal => {
                self.dirty_swapchain = true;
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.dirty_swapchain = true;
            }
            Err(error) => {self.vulkan_context.lifetime.quarantine();egui_app.gpu_error(&format!("GPU presentation failed: {error}; resources quarantined"));},
            _ => {}
        }
    }

    pub fn on_window_event(
        &mut self,
        event: &WindowEvent,
        window: &Window,
        _event_loop: &ActiveEventLoop,
    ) {
        let appended_from=self.egui_winit.egui_input().events.len();
        let response = self.egui_winit.on_window_event(&window, &event);
        if let Ok(mut timeline)=self.pointer_modifiers.lock(){timeline.record_appended(self.egui_winit.egui_input(),appended_from);}
        if response.repaint { self.repaint_at=std::time::Instant::now();window.request_redraw(); }

        match event {
            // Resizing
            WindowEvent::Resized(new_size) => {
                log::debug!("Window was resized. New size is {new_size:?}");
                self.dirty_swapchain = true;
            }
            // Exit
            WindowEvent::CloseRequested => {
                self.close_requested = true;
                self.repaint_at=std::time::Instant::now();window.request_redraw();
            }
            _ => (),
        }
    }

    /// Cold actual-allocation cancellation control. Does not synthesize Vulkan errors.
    pub fn verify_resize_cancellation_for_probe(&mut self,stage:resize::Stage)->Result<resize::Audit,String>{
        let old=(self.swapchain.khr,self.swapchain.render_pass,self.swapchain.image_views.clone(),self.swapchain.framebuffers.clone(),self.render_finished_semaphores.clone());
        let old_pipeline=self.renderer.pipeline_identity_for_probe();
        let old_cad=self.cad2d.as_ref().map(|c|c.descriptor_owner_weak_for_probe().state());
        let bytes=self.memory_budget.ledger.lock().map_err(|_|"Budget lock poisoned")?.total_committed();
        let audit=std::sync::Arc::new(std::sync::Mutex::new(resize::Audit::default()));
        self.dirty_swapchain=true;
        let result=self.resize_transaction_controlled(&resize::Control{cancel:Some(stage),audit:Some(audit.clone())});
        let error=result.err().ok_or("Expected engineering cancellation")?;
        if !error.starts_with("Engineering resize cancellation"){return Err(error);}
        let now=(self.swapchain.khr,self.swapchain.render_pass,self.swapchain.image_views.clone(),self.swapchain.framebuffers.clone(),self.render_finished_semaphores.clone());
        if old!=now||old_pipeline!=self.renderer.pipeline_identity_for_probe()||old_cad!=self.cad2d.as_ref().map(|c|c.descriptor_owner_weak_for_probe().state()){
            return Err("Resize cancellation replaced or destroyed published owners".into());
        }
        if self.vulkan_context.lifetime.uncertain(){return Err("Preparation cancellation poisoned completed device".into());}
        if bytes!=self.memory_budget.ledger.lock().map_err(|_|"Budget lock poisoned")?.total_committed(){return Err("Cancellation changed committed resource ledger".into());}
        if self.swapchain.retired!=(stage!=resize::Stage::BeforeCreate){return Err("Incorrect old-swapchain retirement state".into());}
        let observed=audit.lock().map_err(|_|"Audit lock poisoned")?.clone();
        if observed.created!=observed.destroyed{return Err(format!("Unpublished owner leak: {observed:?}"));}
        // Old owner/last frame retained; acquisition is forbidden after retirement.
        // A successful replacement with oldSwapchain=NULL re-establishes acquisition.
        self.resize_transaction()?;
        if self.swapchain.retired||self.dirty_swapchain{return Err("Retry left retired acquisition state".into());}
        Ok(observed)
    }

    fn report_prepare_error(&self,error:resize::PrepareError)->String{
        if error.device_lost(){self.vulkan_context.lifetime.quarantine();}
        error.to_string()
    }
    fn resize_transaction(&mut self)->Result<(),String>{self.resize_transaction_controlled(&resize::Control::default())}
    fn resize_transaction_controlled(&mut self,control:&resize::Control)->Result<(),String>{
        if !self.wait_gpu_completion(){return Err("Swapchain completion unknown; owners quarantined".into());}
        control.checkpoint(resize::Stage::BeforeCreate)?;
        let mut next=self.swapchain.prepare_replacement(&self.vulkan_context,control).map_err(|e|{if let Some(result)=e.downcast_ref::<vk::Result>(){self.report_prepare_error((*result).into())}else{e.to_string()}})?;
        let candidate=next.value.as_ref().unwrap();
        let mut egui_pipeline=resize::Tracked::new(self.renderer.prepare_render_pass(candidate.render_pass).map_err(|e|match e{egui_ash_renderer::RendererError::Vulkan(v)=>self.report_prepare_error(v.into()),e=>e.to_string()})?,control,vec![(resize::Kind::EguiPipeline,1)]);
        control.checkpoint(resize::Stage::EguiPipeline)?;
        let mut cad_pipeline=if let Some(cad)=&self.cad2d{Some(resize::Tracked::new(cad.prepare_render_pass(candidate.render_pass,&candidate.image_views).map_err(|e|self.report_prepare_error(e))?,control,vec![(resize::Kind::CadPipeline,1),(resize::Kind::CadPool,u64::from(candidate.integer_cad))]))}else{None};
        let mut annotation_pipeline=if let Some(cad)=&self.annotation_cad{Some(cad.prepare_render_pass(candidate.render_pass,&candidate.image_views).map_err(|e|self.report_prepare_error(e))?)}else{None};
        control.checkpoint(resize::Stage::CadPipeline)?;
        let mut sync=UnpublishedSync{device:self.vulkan_context.device.clone(),semaphores:create_present_semaphores(&self.vulkan_context.device,candidate.images.len()).map_err(|e|self.report_prepare_error(e.into()))?,present_fences:Vec::new(),control:control.clone()};
        control.created(resize::Kind::Semaphore,sync.semaphores.len()as u64);
        sync.present_fences=create_present_fences(&self.vulkan_context,candidate.images.len()).map_err(|e|self.report_prepare_error(e.into()))?;
        control.created(resize::Kind::PresentFence,sync.present_fences.len()as u64);
        control.checkpoint(resize::Stage::Sync)?;control.checkpoint(resize::Stage::BeforeCommit)?;
        // CAD lock acquisition is the last fallible publication step. All remaining
        // swaps are infallible; old submitted uses completed before preparation.
        match (&mut self.cad2d,cad_pipeline.as_mut().map(|p|p.take()),&mut self.annotation_cad,annotation_pipeline.take()){
            (Some(cad),Some(prepared),Some(annotation),Some(other))=>cad.commit_render_pass_pair(prepared,annotation,other)?,
            (Some(cad),Some(prepared),_,_)=>cad.commit_render_pass(prepared)?,
            (_,_,Some(annotation),Some(other))=>annotation.commit_render_pass(other)?,
            _=>{},
        }
        self.renderer.commit_render_pass(egui_pipeline.take());
        let mut previous=std::mem::replace(&mut self.swapchain,next.take());
        let old_semaphores=std::mem::replace(&mut self.render_finished_semaphores,std::mem::take(&mut sync.semaphores));
        let old_fences=std::mem::replace(&mut self.present_fences,std::mem::take(&mut sync.present_fences));
        unsafe{for semaphore in old_semaphores{self.vulkan_context.device.destroy_semaphore(semaphore,None);}for fence in old_fences{self.vulkan_context.device.destroy_fence(fence.fence,None);}}
        previous.destroy(&self.vulkan_context.device);
        self.dirty_swapchain=false;
        Ok(())
    }

    fn wait_gpu_completion(&self)->bool{
        if self.presented_without_fence{self.vulkan_context.lifetime.quarantine();eprintln!("Bounded presentation cleanup unavailable: device lacks enabled present-fence feature; retaining WSI ownership");return false;}
        let pending=self.present_fences.iter().filter(|p|p.used).map(|p|p.fence).collect::<Vec<_>>();
        if self.vulkan_context.lifetime.wait_many(&pending,lifetime::WAIT_NS).is_err(){return false;}
        self.vulkan_context.lifetime.drain_graphics()
    }
    fn clean_gpu(&mut self){
        if self.exited{return;}
        if !self.wait_gpu_completion(){drop(self.annotation_timing.take());return;}
        if let Some(world)=&self.world {eprintln!("Native world GPU surface uploads: {}",world.uploads);}
        if let Some(cad)=&self.cad2d {eprintln!("CAD2D metrics: {:?}",cad.stats());eprintln!("Outline arena metrics: {}",cad.outline_arena_stats());}
        if let Some(cad)=&self.annotation_cad{let stats=cad.stats();eprintln!("Native annotation metrics: {}",serde_json::json!({"geometry_uploads":stats.uploads,"geometry_upload_bytes":stats.upload_bytes,"geometry_cached_bytes":stats.cached_bytes,"cache_hits":stats.cache_hits,"draws":stats.draws,"rolled_back_buffers":stats.rolled_back_buffers,"query_uploads":self.world.as_ref().map_or(0,|world|world.annotation_query_uploads_for_probe())}));}
        drop(self.annotation_timing.take());
        drop(self.world.take());drop(self.annotation_cad.take());drop(self.cad2d.take());
        if self.vulkan_context.lifetime.uncertain(){return;}
        unsafe{
            std::mem::ManuallyDrop::drop(&mut self.renderer);
            self.vulkan_context.device.destroy_fence(self.fence,None);
            self.vulkan_context.device.destroy_semaphore(self.image_available_semaphore,None);
            for semaphore in self.render_finished_semaphores.drain(..){self.vulkan_context.device.destroy_semaphore(semaphore,None);}
            for present in self.present_fences.drain(..){self.vulkan_context.device.destroy_fence(present.fence,None);}
            self.swapchain.destroy(&self.vulkan_context.device);
            self.vulkan_context.device.free_command_buffers(self.vulkan_context.command_pool,&[self.command_buffer]);
        }
        self.exited=true;
        eprintln!("Native bounded GPU cleanup complete; present_fences_enabled={}; committed_bytes={}",self.vulkan_context.present_fences_enabled,self.memory_budget.ledger.lock().unwrap_or_else(|p|p.into_inner()).total_committed());
    }
    fn on_exit<A:App>(&mut self,egui_app:&mut A){
        eprintln!("{}",self.memory_budget.diagnostics());
        if !self.wait_gpu_completion(){eprintln!("GPU shutdown quarantined: completion unknown");return;}
        egui_app.clean(&self.vulkan_context);self.clean_gpu();
    }

}

impl Drop for System {fn drop(&mut self){self.clean_gpu();if !self.exited{eprintln!("Retaining quarantined egui renderer and WSI resources");self.vulkan_context.lifetime.retain(self.memory_budget.clone());self.vulkan_context.lifetime.retain(unsafe{std::mem::ManuallyDrop::take(&mut self.renderer)});self.vulkan_context.lifetime.retain((self.swapchain.loader.clone(),self.swapchain.khr,self.swapchain.render_pass,std::mem::take(&mut self.swapchain.framebuffers),std::mem::take(&mut self.swapchain.image_views),std::mem::take(&mut self.render_finished_semaphores),std::mem::take(&mut self.present_fences),self.image_available_semaphore,self.fence,self.command_buffer));}}}
// Startup has no submitted work: reverse local drop order releases these
// handles before VulkanContext destroys the device. Published System owns them.
struct UnpublishedStartup{device:Device,pool:vk::CommandPool,command:Option<vk::CommandBuffer>,acquire:Option<vk::Semaphore>,fence:Option<vk::Fence>,control:resize::Control}
impl Drop for UnpublishedStartup{fn drop(&mut self){unsafe{
 if let Some(fence)=self.fence.take(){self.device.destroy_fence(fence,None);self.control.destroyed(resize::Kind::GraphicsFence,1);}
 if let Some(semaphore)=self.acquire.take(){self.device.destroy_semaphore(semaphore,None);self.control.destroyed(resize::Kind::AcquireSemaphore,1);}
 if let Some(command)=self.command.take(){self.device.free_command_buffers(self.pool,&[command]);self.control.destroyed(resize::Kind::CommandBuffer,1);}
}}}
struct UnpublishedSync{device:Device,semaphores:Vec<vk::Semaphore>,present_fences:Vec<PresentFence>,control:resize::Control}
impl Drop for UnpublishedSync{fn drop(&mut self){let sem=self.semaphores.len();let fences=self.present_fences.len();unsafe{for semaphore in self.semaphores.drain(..){self.device.destroy_semaphore(semaphore,None);}for fence in self.present_fences.drain(..){self.device.destroy_fence(fence.fence,None);}}self.control.destroyed(resize::Kind::Semaphore,sem as u64);self.control.destroyed(resize::Kind::PresentFence,fences as u64);}}

struct PresentFence{fence:vk::Fence,used:bool,serial:u64}
fn create_present_fences(context:&VulkanContext,count:usize)->Result<Vec<PresentFence>,vk::Result>{
 if !context.present_fences_enabled{return Ok(Vec::new());}
 let mut fences=Vec::new();for _ in 0..count{match unsafe{context.device.create_fence(&vk::FenceCreateInfo::default(),None)}{Ok(fence)=>fences.push(PresentFence{fence,used:false,serial:0}),Err(error)=>{unsafe{for p in fences{context.device.destroy_fence(p.fence,None);}}return Err(error);}}}Ok(fences)
}

fn create_present_semaphores(device:&Device,count:usize)->Result<Vec<vk::Semaphore>,vk::Result> {
    let mut semaphores=Vec::with_capacity(count);
    for _ in 0..count { match unsafe {device.create_semaphore(&vk::SemaphoreCreateInfo::default(),None)} {
        Ok(semaphore)=>semaphores.push(semaphore),
        Err(error)=>{unsafe {for semaphore in semaphores {device.destroy_semaphore(semaphore,None);}}return Err(error);}
    }}
    Ok(semaphores)
}

pub struct VulkanContext {
    startup_audit:resize::Control,
    _entry: Option<Entry>,
    pub lifetime: std::sync::Arc<lifetime::Lifetime>,
    pub instance: Instance,
    debug_utils: debug_utils::Instance,
    debug_utils_messenger: vk::DebugUtilsMessengerEXT,
    surface: surface::Instance,
    surface_khr: vk::SurfaceKHR,
    pub physical_device: vk::PhysicalDevice,
    graphics_q_index: u32,
    present_q_index: u32,
    pub device: Device,
    pub graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    pub command_pool: vk::CommandPool,
    pub present_fences_enabled:bool,
    pub texture_bc_enabled:bool,
    pub source_depth_clip_enabled:bool,
    pub source_discard_terminate_enabled:bool,
    pub transfer_timeout_ns:u64,
    pub diagnostic_fail_fast:bool,
}

impl VulkanContext {
    pub fn wait_transfer(&self,fence:vk::Fence)->Result<(),vk::Result> {
        let result=self.lifetime.wait(fence,self.transfer_timeout_ns);
        if result==Err(vk::Result::TIMEOUT)&&self.diagnostic_fail_fast {
            eprintln!("GPU diagnostic fence exceeded {}ns; exiting without freeing potentially pending resources",self.transfer_timeout_ns);
            std::process::exit(124);
        }
        result
    }
    pub fn new(window: &Window, name: &str) -> Result<Self, Box<dyn Error>> {
        Self::new_controlled(window,name,&resize::Control::default())
    }
    fn new_controlled(window:&Window,name:&str,control:&resize::Control)->Result<Self,Box<dyn Error>>{
        // Vulkan instance
        let entry = unsafe { Entry::load()? };
        let (instance, debug_utils, debug_utils_messenger) =
            create_vulkan_instance(&entry, window, name)?;
        let mut pending=UnpublishedContext{instance:Some(instance.clone()),debug:Some((debug_utils.clone(),debug_utils_messenger)),surface:None,device:None,pool:None,control:control.clone()};
        control.created(resize::Kind::Instance,1);control.created(resize::Kind::DebugMessenger,1);control.checkpoint(resize::Stage::StartupInstance)?;
        // Vulkan surface
        let surface = surface::Instance::new(&entry, &instance);
        let surface_khr = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?
        };

        pending.surface=Some((surface.clone(),surface_khr));control.created(resize::Kind::Surface,1);control.checkpoint(resize::Stage::StartupSurface)?;
        // Vulkan physical device and queue families indices (graphics and present)
        let (physical_device, graphics_q_index, present_q_index) =
            create_vulkan_physical_device_and_get_graphics_and_present_qs_indices(
                &instance,
                &surface,
                surface_khr,
            )?;

        // Vulkan logical device and queues
        let (device, graphics_queue, present_queue,present_fences_enabled,texture_bc_enabled,source_depth_clip_enabled,source_discard_terminate_enabled) =
            create_vulkan_device_and_graphics_and_present_qs(
                &instance,
                physical_device,
                graphics_q_index,
                present_q_index,
                surface_maintenance_supported(&entry)?,
            )?;

        pending.device=Some(device.clone());control.created(resize::Kind::Device,1);control.checkpoint(resize::Stage::StartupDevice)?;
        // Command pool & buffer
        let command_pool = {
            let command_pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_q_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            unsafe { device.create_command_pool(&command_pool_info, None)? }
        };

        pending.pool=Some(command_pool);control.created(resize::Kind::CommandPool,1);control.checkpoint(resize::Stage::StartupPool)?;
        pending.instance=None;pending.debug=None;pending.surface=None;pending.device=None;pending.pool=None;
        Ok(Self {
            startup_audit:control.clone(),
            lifetime:std::sync::Arc::new(lifetime::Lifetime::new(device.clone(),graphics_queue)),
            _entry: Some(entry),
            instance,
            debug_utils,
            debug_utils_messenger,
            surface,
            surface_khr,
            physical_device,
            graphics_q_index,
            present_q_index,
            device,
            graphics_queue,
            present_queue,
            command_pool,
            present_fences_enabled,
            texture_bc_enabled,
            source_depth_clip_enabled,
            source_discard_terminate_enabled,
            transfer_timeout_ns:lifetime::WAIT_NS,
            diagnostic_fail_fast:false,
        })
    }
}

// No queues have been submitted during context construction. Guard every raw
// object before the next fallible call, keeping the loaded Entry alive until last.
struct UnpublishedContext{instance:Option<Instance>,debug:Option<(debug_utils::Instance,vk::DebugUtilsMessengerEXT)>,surface:Option<(surface::Instance,vk::SurfaceKHR)>,device:Option<Device>,pool:Option<vk::CommandPool>,control:resize::Control}
impl Drop for UnpublishedContext{fn drop(&mut self){unsafe{
 if let Some(device)=self.device.take(){if let Some(pool)=self.pool.take(){device.destroy_command_pool(pool,None);self.control.destroyed(resize::Kind::CommandPool,1);}device.destroy_device(None);self.control.destroyed(resize::Kind::Device,1);}
 if let Some((surface,handle))=self.surface.take(){surface.destroy_surface(handle,None);self.control.destroyed(resize::Kind::Surface,1);}
 if let Some((debug,handle))=self.debug.take(){debug.destroy_debug_utils_messenger(handle,None);self.control.destroyed(resize::Kind::DebugMessenger,1);}
 if let Some(instance)=self.instance.take(){instance.destroy_instance(None);self.control.destroyed(resize::Kind::Instance,1);}
}}}
impl Drop for VulkanContext {
    fn drop(&mut self) {
        log::debug!("Destroying Vulkan Context");
        if self.lifetime.uncertain(){eprintln!("Retaining quarantined Vulkan device, instance and loader; GPU completion unknown");if let Some(entry)=self._entry.take(){std::mem::forget(entry);}return;}
        unsafe {
            self.device.destroy_command_pool(self.command_pool, None);self.startup_audit.destroyed(resize::Kind::CommandPool,1);
            self.device.destroy_device(None);self.startup_audit.destroyed(resize::Kind::Device,1);
            if self.surface_khr!=vk::SurfaceKHR::null(){self.surface.destroy_surface(self.surface_khr, None);self.startup_audit.destroyed(resize::Kind::Surface,1);}
            if self.debug_utils_messenger!=vk::DebugUtilsMessengerEXT::null(){self.debug_utils.destroy_debug_utils_messenger(self.debug_utils_messenger, None);self.startup_audit.destroyed(resize::Kind::DebugMessenger,1);}
            self.instance.destroy_instance(None);self.startup_audit.destroyed(resize::Kind::Instance,1);
        }
    }
}

struct Swapchain {
    loader: swapchain::Device,
    extent: vk::Extent2D,
    khr: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    srgb: bool,
    format:vk::Format,
    retired:bool,
    image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    integer_cad:bool,
    stencil_enabled:bool,stencil:Option<annotation_stencil::Target>,budget:std::sync::Arc<budget::VulkanBudget>,
}

struct UnpublishedSwapchain{device:Device,value:Option<Swapchain>,control:resize::Control}
impl UnpublishedSwapchain{fn take(&mut self)->Swapchain{self.value.take().unwrap()}}
impl Drop for UnpublishedSwapchain{fn drop(&mut self){if let Some(value)=&mut self.value{
 let counts=[(resize::Kind::Swapchain,u64::from(value.khr!=vk::SwapchainKHR::null())),(resize::Kind::ImageView,value.image_views.len()as u64),(resize::Kind::RenderPass,u64::from(value.render_pass!=vk::RenderPass::null())),(resize::Kind::Framebuffer,value.framebuffers.len()as u64)];
 value.destroy(&self.device);for(k,n)in counts{self.control.destroyed(k,n);}
}}}

impl Swapchain {
    fn build(context:&VulkanContext,budget:std::sync::Arc<budget::VulkanBudget>,stencil_enabled:bool,integer:bool,old:vk::SwapchainKHR,retired:&mut bool,format:Option<vk::Format>,control:&resize::Control)->Result<UnpublishedSwapchain,Box<dyn Error>>{
        let mut pending=create_vulkan_swapchain(context,budget.clone(),stencil_enabled,integer,old,retired,format,control)?;
        let value=pending.value.as_mut().unwrap();
        if stencil_enabled{value.stencil=Some(annotation_stencil::Target::new(context,budget,value.extent)?);}
        value.render_pass=create_vulkan_render_pass(&context.device,value.format,integer,value.stencil.as_ref().map(|s|s.format))?;
        control.created(resize::Kind::RenderPass,1);control.checkpoint(resize::Stage::RenderPass)?;
        value.framebuffers=create_vulkan_framebuffers(&context.device,value.render_pass,value.extent,&value.image_views,value.stencil.as_ref().map(|s|s.view),control)?;
        Ok(pending)
    }
    fn prepare_replacement(&mut self,context:&VulkanContext,control:&resize::Control)->Result<UnpublishedSwapchain,Box<dyn Error>>{
        // Vulkan retires oldSwapchain at creation, including failed creation.
        let old=if self.retired{vk::SwapchainKHR::null()}else{self.khr};
        Self::build(context,self.budget.clone(),self.stencil_enabled,self.integer_cad,old,&mut self.retired,Some(self.format),control)
    }

    fn destroy(&mut self, device: &Device) {
        unsafe {
            self.framebuffers
                .iter()
                .for_each(|fb| device.destroy_framebuffer(*fb, None));
            self.framebuffers.clear();self.stencil=None;
            if self.render_pass!=vk::RenderPass::null(){device.destroy_render_pass(self.render_pass, None);}
            self.image_views
                .iter()
                .for_each(|v| device.destroy_image_view(*v, None));
            self.image_views.clear();
            if self.khr!=vk::SwapchainKHR::null(){self.loader.destroy_swapchain(self.khr, None);self.khr=vk::SwapchainKHR::null();}
            self.render_pass=vk::RenderPass::null();
        }
    }
}

fn create_window(title: &str, event_loop: &ActiveEventLoop) -> Result<Window, Box<dyn Error>> {
    log::debug!("Creating window");

    let window = event_loop.create_window(
        Window::default_attributes()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT)),
    )?;

    Ok(window)
}

fn surface_maintenance_supported(entry: &Entry) -> Result<bool, vk::Result> {
    let extensions = unsafe { entry.enumerate_instance_extension_properties(None)? };
    Ok([ash::ext::surface_maintenance1::NAME, ash::khr::get_surface_capabilities2::NAME]
        .iter().all(|required| extensions.iter().any(|e| unsafe {
            CStr::from_ptr(e.extension_name.as_ptr()) == *required
        })))
}

fn create_vulkan_instance(
    entry: &Entry,
    window: &Window,
    title: &str,
) -> Result<(Instance, debug_utils::Instance, vk::DebugUtilsMessengerEXT), Box<dyn Error>> {
    log::debug!("Creating vulkan instance");
    // Vulkan instance
    let app_name = CString::new(title)?;
    let app_info = vk::ApplicationInfo::default()
        .application_name(app_name.as_c_str())
        .application_version(vk::make_api_version(0, 0, 1, 0))
        .engine_name(c"No Engine")
        .engine_version(vk::make_api_version(0, 0, 1, 0))
        .api_version(vk::make_api_version(0, 1, 1, 0));

    let mut extension_names =
        ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())?.to_vec();
    extension_names.push(debug_utils::NAME.as_ptr());
    // EXT_surface_maintenance1 requires KHR_get_surface_capabilities2.
    if surface_maintenance_supported(entry)? {
        extension_names.push(ash::khr::get_surface_capabilities2::NAME.as_ptr());
        extension_names.push(ash::ext::surface_maintenance1::NAME.as_ptr());
    }

    let create_flags=vk::InstanceCreateFlags::default();

    let instance_create_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .flags(create_flags)
        .enabled_extension_names(&extension_names);

    let instance = unsafe { entry.create_instance(&instance_create_info, None)? };

    // Vulkan debug report
    let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .flags(vk::DebugUtilsMessengerCreateFlagsEXT::empty())
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
        )
        .pfn_user_callback(Some(vulkan_debug_callback));
    let debug_utils = debug_utils::Instance::new(entry, &instance);
    let debug_utils_messenger = match unsafe{debug_utils.create_debug_utils_messenger(&create_info,None)}{
        Ok(value)=>value,Err(error)=>{unsafe{instance.destroy_instance(None)};return Err(error.into());}
    };

    Ok((instance, debug_utils, debug_utils_messenger))
}

unsafe extern "system" fn vulkan_debug_callback(
    flag: vk::DebugUtilsMessageSeverityFlagsEXT,
    typ: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> vk::Bool32 {
    use vk::DebugUtilsMessageSeverityFlagsEXT as Flag;

    let message = CStr::from_ptr((*p_callback_data).p_message);
    match flag {
        Flag::VERBOSE => log::debug!("{typ:?} - {message:?}"),
        Flag::INFO => log::info!("{typ:?} - {message:?}"),
        Flag::WARNING => log::warn!("{typ:?} - {message:?}"),
        _ => log::error!("{typ:?} - {message:?}"),
    }
    vk::FALSE
}

fn create_vulkan_physical_device_and_get_graphics_and_present_qs_indices(
    instance: &Instance,
    surface: &surface::Instance,
    surface_khr: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32, u32), Box<dyn Error>> {
    log::debug!("Creating vulkan physical device");
    let devices = unsafe { instance.enumerate_physical_devices()? };
    let requested=std::env::var("INGETRAZO_VULKAN_DEVICE").ok();
    let mut suitable=Vec::new();
    for (index,device) in devices.into_iter().enumerate() {
        let properties=unsafe {instance.get_physical_device_properties(device)};
        let name=unsafe {CStr::from_ptr(properties.device_name.as_ptr())}.to_string_lossy().into_owned();
        let families=unsafe {instance.get_physical_device_queue_family_properties(device)};
        let present=families.iter().enumerate().map(|(i,f)|if f.queue_count==0 {Ok(false)}else{unsafe {surface.get_physical_device_surface_support(device,i as u32,surface_khr)}}).collect::<Result<Vec<_>,_>>()?;
        let Some(queues)=queues::select(&families,&present) else {eprintln!("Vulkan adapter {index} {name}: no suitable graphics/present queues");continue;};
        let extensions=unsafe {instance.enumerate_device_extension_properties(device)}?;
        if !extensions.iter().any(|e|unsafe {CStr::from_ptr(e.extension_name.as_ptr())}==swapchain::NAME) {continue;}
        if unsafe {surface.get_physical_device_surface_formats(device,surface_khr)}?.is_empty()||unsafe {surface.get_physical_device_surface_present_modes(device,surface_khr)}?.is_empty(){continue;}
        eprintln!("Suitable Vulkan adapter {index}: {name}; {:?}; queues {:?}",properties.device_type,queues);
        if requested.as_ref().is_some_and(|r|r!=&index.to_string()&&!name.to_lowercase().contains(&r.to_lowercase())) {continue;}
        let memory=unsafe {instance.get_physical_device_memory_properties(device)};
        let local=memory.memory_heaps[..memory.memory_heap_count as usize].iter().filter(|h|h.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL)).map(|h|h.size).sum::<u64>();
        suitable.push((queues::rank(properties.device_type),local,index,device,queues));
    }
    if requested.is_some()&&suitable.len()>1 {return Err("Vulkan device override matches multiple adapters; use enumeration index".into());}
    suitable.sort_by_key(|(rank,bytes,index,_,_)|(*rank,*bytes,std::cmp::Reverse(*index)));
    let (_,_,_,device,queues)=suitable.pop().ok_or("No suitable Vulkan adapter (or override did not match)")?;
    let graphics=queues.graphics;let present=queues.present;

    unsafe {
        let props = instance.get_physical_device_properties(device);
        let device_name = CStr::from_ptr(props.device_name.as_ptr());
        let features=instance.get_physical_device_features(device);
        let memory=instance.get_physical_device_memory_properties(device);
        let extensions=instance.enumerate_device_extension_properties(device)?;
        let has=|name:&CStr| extensions.iter().any(|e|CStr::from_ptr(e.extension_name.as_ptr())==name);
        eprintln!("Native Vulkan adapter: {:?}; API {}.{}.{}; max texture {}; BC support {}; memory-budget extension {}; acceleration-structure extension {}; ray-tracing-pipeline extension {}",
            device_name,vk::api_version_major(props.api_version),vk::api_version_minor(props.api_version),vk::api_version_patch(props.api_version),props.limits.max_image_dimension2_d,features.texture_compression_bc!=0,
            has(ash::ext::memory_budget::NAME),has(ash::khr::acceleration_structure::NAME),has(ash::khr::ray_tracing_pipeline::NAME));
        for (i,heap) in memory.memory_heaps[..memory.memory_heap_count as usize].iter().enumerate() { eprintln!("Vulkan heap {i}: {} bytes; {:?}",heap.size,heap.flags); }

    }

    Ok((device, graphics, present))
}

fn create_vulkan_device_and_graphics_and_present_qs(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    graphics_q_index: u32,
    present_q_index: u32,
    surface_maintenance_enabled:bool,
) -> Result<(Device, vk::Queue, vk::Queue,bool,bool,bool,bool), Box<dyn Error>> {
    log::debug!("Creating vulkan device and graphics and present queues");
    let queue_priorities = [1.0f32];
    let queue_create_infos = {
        let mut indices = vec![graphics_q_index, present_q_index];
        indices.dedup();

        indices
            .iter()
            .map(|index| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(*index)
                    .queue_priorities(&queue_priorities)
            })
            .collect::<Vec<_>>()
    };

    let mut device_extensions_ptrs = vec![
        swapchain::NAME.as_ptr(),
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        ash::khr::portability_subset::NAME.as_ptr(),
    ];

    let extensions=unsafe{instance.enumerate_device_extension_properties(physical_device)?};
    let (source_depth_clip_enabled,mut source_clip,mut source_dynamic)=source_depth::negotiate(instance,physical_device,&extensions);
    let (source_discard_terminate_enabled,mut source_discard)=source_discard::negotiate(instance,physical_device,&extensions);
    if source_discard_terminate_enabled{device_extensions_ptrs.push(ash::khr::shader_terminate_invocation::NAME.as_ptr());}
    if source_depth_clip_enabled{device_extensions_ptrs.extend([ash::ext::depth_clip_control::NAME.as_ptr(),ash::ext::extended_dynamic_state3::NAME.as_ptr()]);}
    let has_maintenance=surface_maintenance_enabled&&extensions.iter().any(|e|unsafe{CStr::from_ptr(e.extension_name.as_ptr())}==ash::ext::swapchain_maintenance1::NAME);
    let mut maintenance=vk::PhysicalDeviceSwapchainMaintenance1FeaturesEXT::default();
    if has_maintenance{let mut query=vk::PhysicalDeviceFeatures2::default().push_next(&mut maintenance);unsafe{instance.get_physical_device_features2(physical_device,&mut query);}}
    let enabled=has_maintenance&&maintenance.swapchain_maintenance1!=0&&std::env::var_os("INGETRAZO_DISABLE_PRESENT_FENCES").is_none();
    if enabled{device_extensions_ptrs.push(ash::ext::swapchain_maintenance1::NAME.as_ptr());}
    eprintln!("Native present-fence negotiation: extension={has_maintenance} feature={} enabled={enabled}",maintenance.swapchain_maintenance1!=0);
    let available=unsafe{instance.get_physical_device_features(physical_device)};
    let texture_bc_enabled=available.texture_compression_bc!=0;
    let features=vk::PhysicalDeviceFeatures::default().shader_clip_distance(available.shader_clip_distance!=0).texture_compression_bc(texture_bc_enabled);
    let mut device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_create_infos)
        .enabled_extension_names(&device_extensions_ptrs)
        .enabled_features(&features);

    if enabled{device_create_info=device_create_info.push_next(&mut maintenance);}
    if source_depth_clip_enabled{device_create_info=device_create_info.push_next(&mut source_clip).push_next(&mut source_dynamic);}
    if source_discard_terminate_enabled{device_create_info=device_create_info.push_next(&mut source_discard);}
    let device = unsafe { instance.create_device(physical_device, &device_create_info, None)? };
    let graphics_queue = unsafe { device.get_device_queue(graphics_q_index, 0) };
    let present_queue = unsafe { device.get_device_queue(present_q_index, 0) };

    Ok((device, graphics_queue, present_queue,enabled,texture_bc_enabled,source_depth_clip_enabled,source_discard_terminate_enabled))
}

fn create_vulkan_swapchain(
    vulkan_context: &VulkanContext,
    budget:std::sync::Arc<budget::VulkanBudget>,stencil_enabled:bool,
    integer_cad:bool,old:vk::SwapchainKHR,retired:&mut bool,expected_format:Option<vk::Format>,control:&resize::Control,
) -> Result<UnpublishedSwapchain, Box<dyn Error>> {
    log::debug!("Creating vulkan swapchain");
    // Swapchain format
    let format = {
        let formats = unsafe {
            vulkan_context.surface.get_physical_device_surface_formats(
                vulkan_context.physical_device,
                vulkan_context.surface_khr,
            )?
        };

        select_encoded_surface_format(&formats)
            .ok_or("Surface provides no encoded UNORM/SRGB_NONLINEAR presentation format; linear sRGB blending cannot reproduce document composition")?

    };
    if expected_format.is_some_and(|expected|expected!=format.format){return Err("Swapchain format changed; backend recreation required".into());}
    log::debug!("Swapchain format: {format:?}");

    // Swapchain present mode
    let present_mode = {
        let present_modes = unsafe {
            vulkan_context
                .surface
                .get_physical_device_surface_present_modes(
                    vulkan_context.physical_device,
                    vulkan_context.surface_khr,
                )?
        };
        if present_modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            vk::PresentModeKHR::IMMEDIATE
        } else {
            vk::PresentModeKHR::FIFO
        }
    };
    log::debug!("Swapchain present mode: {present_mode:?}");

    let capabilities = unsafe {
        vulkan_context
            .surface
            .get_physical_device_surface_capabilities(
                vulkan_context.physical_device,
                vulkan_context.surface_khr,
            )?
    };

    let usage=cad_attachment_usage(capabilities.supported_usage_flags,integer_cad)?;
    eprintln!("CAD2D attachment usage: requested={usage:?} supported={:?}",capabilities.supported_usage_flags);
    // Swapchain extent
    let extent = {
        if capabilities.current_extent.width != std::u32::MAX {
            capabilities.current_extent
        } else {
            let min = capabilities.min_image_extent;
            let max = capabilities.max_image_extent;
            let width = WIDTH.min(max.width).max(min.width);
            let height = HEIGHT.min(max.height).max(min.height);
            vk::Extent2D { width, height }
        }
    };
    log::debug!("Swapchain extent: {extent:?}");

    // Swapchain image count
    let image_count = capabilities.min_image_count;
    log::debug!("Swapchain image count: {image_count:?}");

    // Swapchain
    let families_indices = [
        vulkan_context.graphics_q_index,
        vulkan_context.present_q_index,
    ];
    let create_info = {
        let mut builder = vk::SwapchainCreateInfoKHR::default()
            .surface(vulkan_context.surface_khr).old_swapchain(old)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(usage);

        builder = if vulkan_context.graphics_q_index != vulkan_context.present_q_index {
            builder
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&families_indices)
        } else {
            builder.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        };

        builder
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
    };

    let swapchain = swapchain::Device::new(&vulkan_context.instance, &vulkan_context.device);
    if old!=vk::SwapchainKHR::null(){*retired=true;}
    let swapchain_khr = unsafe { swapchain.create_swapchain(&create_info, None)? };

    // Guard every created view and swapchain before the next fallible call.
    let mut pending=UnpublishedSwapchain{device:vulkan_context.device.clone(),control:control.clone(),value:Some(Swapchain{
        loader:swapchain,khr:swapchain_khr,extent,format:format.format,
        srgb:matches!(format.format,vk::Format::R8G8B8A8_SRGB|vk::Format::B8G8R8A8_SRGB),retired:false,
        integer_cad,stencil_enabled,stencil:None,budget,images:Vec::new(),image_views:Vec::new(),render_pass:vk::RenderPass::null(),framebuffers:Vec::new(),
    })};
    control.created(resize::Kind::Swapchain,1);
    let value=pending.value.as_mut().unwrap();
    value.images=unsafe{value.loader.get_swapchain_images(value.khr)?};
    for image in &value.images{
        let info=vk::ImageViewCreateInfo::default().image(*image).view_type(vk::ImageViewType::TYPE_2D)
            .format(format.format).subresource_range(vk::ImageSubresourceRange{aspect_mask:vk::ImageAspectFlags::COLOR,base_mip_level:0,level_count:1,base_array_layer:0,layer_count:1});
        value.image_views.push(unsafe{vulkan_context.device.create_image_view(&info,None)?});
        control.created(resize::Kind::ImageView,1);control.checkpoint(resize::Stage::ImageView(value.image_views.len()))?;
    }
    Ok(pending)
}

fn create_vulkan_render_pass(
    device: &Device,
    format: vk::Format,
    integer_cad:bool,stencil_format:Option<vk::Format>,
) -> Result<vk::RenderPass, Box<dyn Error>> {
    log::debug!("Creating vulkan render pass");
    let mut attachment_descs = vec![vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

    if let Some(format)=stencil_format{attachment_descs.push(vk::AttachmentDescription::default().format(format).samples(vk::SampleCountFlags::TYPE_1).load_op(vk::AttachmentLoadOp::DONT_CARE).store_op(vk::AttachmentStoreOp::DONT_CARE).stencil_load_op(vk::AttachmentLoadOp::CLEAR).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL));}
    let stencil_ref=vk::AttachmentReference::default().attachment(1).layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
    let attachment_layout=if integer_cad{vk::ImageLayout::GENERAL}else{vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL};
    let color_attachment_refs = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(attachment_layout)];
    let input_attachment_refs=if integer_cad{color_attachment_refs.to_vec()}else{Vec::new()};

    let mut subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs).input_attachments(&input_attachment_refs);
    if stencil_format.is_some(){subpass=subpass.depth_stencil_attachment(&stencil_ref);}
    let subpass_descs=[subpass];

    let mut subpass_deps = vec![vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )];

    if stencil_format.is_some(){
        subpass_deps[0].dst_stage_mask|=vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS|vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
        subpass_deps[0].dst_access_mask|=vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ|vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
    }
    if integer_cad{subpass_deps.push(vk::SubpassDependency::default().src_subpass(0).dst_subpass(0).src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT).dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER).src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE).dst_access_mask(vk::AccessFlags::INPUT_ATTACHMENT_READ).dependency_flags(vk::DependencyFlags::BY_REGION));}
    if stencil_format.is_some(){subpass_deps.push(vk::SubpassDependency::default().src_subpass(0).dst_subpass(0).src_stage_mask(vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS|vk::PipelineStageFlags::LATE_FRAGMENT_TESTS).dst_stage_mask(vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS|vk::PipelineStageFlags::LATE_FRAGMENT_TESTS).src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE).dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ|vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE).dependency_flags(vk::DependencyFlags::BY_REGION));}
    let render_pass_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachment_descs)
        .subpasses(&subpass_descs)
        .dependencies(&subpass_deps);

    Ok(unsafe { device.create_render_pass(&render_pass_info, None)? })
}

fn create_vulkan_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    image_views: &[vk::ImageView],stencil_view:Option<vk::ImageView>,control:&resize::Control,
) -> Result<Vec<vk::Framebuffer>, Box<dyn Error>> {
    log::debug!("Creating vulkan framebuffers");
    let mut framebuffers=Vec::new();
    for view in image_views{
        let mut attachments=vec![*view];if let Some(stencil)=stencil_view{attachments.push(stencil);}
        let info=vk::FramebufferCreateInfo::default().render_pass(render_pass).attachments(&attachments).width(extent.width).height(extent.height).layers(1);
        match unsafe{device.create_framebuffer(&info,None)}{
            Ok(framebuffer)=>{framebuffers.push(framebuffer);control.created(resize::Kind::Framebuffer,1);
                if let Err(error)=control.checkpoint(resize::Stage::Framebuffer(framebuffers.len())){let n=framebuffers.len();unsafe{for framebuffer in framebuffers{device.destroy_framebuffer(framebuffer,None);}}control.destroyed(resize::Kind::Framebuffer,n as u64);return Err(error.into());}},
            Err(error)=>{let n=framebuffers.len();unsafe{for framebuffer in framebuffers{device.destroy_framebuffer(framebuffer,None);}}control.destroyed(resize::Kind::Framebuffer,n as u64);return Err(error.into());}
        }
    }
    Ok(framebuffers)
}

fn record_command_buffers(
    device: &Device,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    framebuffer: vk::Framebuffer,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    pixels_per_point: f32,
    renderer: &mut Renderer,

    clipped_primitives: &[ClippedPrimitive],
    world:Option<&mut world::World>,serial:u64,timing:Option<vk::QueryPool>,
) -> Result<Option<String>, Box<dyn Error>> {
    unsafe { device.reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty())? };

    let command_buffer_begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::SIMULTANEOUS_USE);
    unsafe { device.begin_command_buffer(command_buffer, &command_buffer_begin_info)? };
    if let Some(pool)=timing{unsafe{device.cmd_reset_query_pool(command_buffer,pool,0,2)}}
    if let Some(world)=world {world.record(command_buffer,serial)?;world.record_annotations_if_prepared(command_buffer,serial)?;}

    let render_pass_begin_info = vk::RenderPassBeginInfo::default()
        .render_pass(render_pass)
        .framebuffer(framebuffer)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        })
        .clear_values(&[vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.007, 0.007, 0.007, 1.0],
            },
        },vk::ClearValue{depth_stencil:vk::ClearDepthStencilValue{depth:1.,stencil:0}}]);

    unsafe {
        device.cmd_begin_render_pass(
            command_buffer,
            &render_pass_begin_info,
            vk::SubpassContents::INLINE,
        )
    };

    if let Some(pool)=timing{unsafe{device.cmd_write_timestamp(command_buffer,vk::PipelineStageFlags::BOTTOM_OF_PIPE,pool,0)}}
    let draw_error=renderer.cmd_draw(command_buffer,extent,pixels_per_point,clipped_primitives).err().map(|e|e.to_string());
    if let Some(pool)=timing{unsafe{device.cmd_write_timestamp(command_buffer,vk::PipelineStageFlags::BOTTOM_OF_PIPE,pool,1)}}
    if draw_error.is_some() {
        // Previous fence is complete and this command buffer has not submitted.
        // Abort all partial callback commands, then consume the acquired frame
        // through a clear-only pass. The host reports failure and retries.
        unsafe {
            device.reset_command_buffer(command_buffer,vk::CommandBufferResetFlags::empty())?;
            device.begin_command_buffer(command_buffer,&command_buffer_begin_info)?;
            if let Some(pool)=timing{device.cmd_reset_query_pool(command_buffer,pool,0,2);}
            device.cmd_begin_render_pass(command_buffer,&render_pass_begin_info,vk::SubpassContents::INLINE);
        }
    }

    unsafe { device.cmd_end_render_pass(command_buffer) };

    unsafe { device.end_command_buffer(command_buffer)? };

    Ok(draw_error)
}

impl App for crate::IngeTrazoApp {
    fn title() -> &'static str { "IngeTrazo (native Vulkan)" }
    fn new(system: &mut System, initial_file: Option<std::path::PathBuf>) -> Self {
        let mut app=Self::new();
        match system.enable_cad2d(){Ok(renderer)=>app.native_composer.factory=Some(renderer.factory().into()),Err(error)=>app.gpu_error(&error)}
        app.native_gpu_viewport_enabled=std::env::var_os("INGETRAZO_VULKAN_WORLD").is_some();
        app.system_clipboard=arboard::Clipboard::new().ok();
        app.attach_session_store(crate::session_state::SessionStore::user_default());
        if let Some(path)=initial_file { app.open_document(&path); }
        app
    }
    fn prepare_gpu(&mut self,system:&mut System,_resize:bool) {
        // Collect a stable slot before UI/tool dispatch. A tool can acquire or
        // cancel during ctx.run; resolving this slot after post-UI preparation
        // preserves current-frame membership without capturing an old handle.
        self.native_gpu_annotation_shape=self.native_gpu_viewport_enabled.then(annotation_slot);
        if !self.native_gpu_viewport_enabled||self.scene.active_sheet_index.is_some(){
            if !_resize&&system.annotation_cad.is_some(){let result=system.clear_annotation_overlay();system.annotation_prepare_error=result.as_ref().err().cloned();match result{Ok(())=>system.annotation_ready_shape=None,Err(error)=>self.gpu_error(&error)}}
            return;
        }
        let Some(rect)=self.native_gpu_viewport_rect else {return;};
        let mut resize_transaction=None;
        let result:Result<(),String>=(|| {
            if system.world.is_none(){system.world=Some(world::World::new(&system.vulkan_context,system.memory_budget.clone())?);}
            let world=system.world.as_mut().unwrap();
            // Layout can change independently of OS resize; the current rect
            // is authoritative. World.resize returns the cached texture in O(1)
            // if extent is unchanged, with no allocation/upload/copy.
            let scale=system.egui_ctx.pixels_per_point();let [width,height]=crate::viewport_extent::scene_extent([rect.width() as f64,rect.height() as f64],scale as f64)?;let extent=vk::Extent2D{width,height};if self.native_gpu_viewport_texture.is_none(){self.native_gpu_viewport_texture=Some(world.resize(&system.vulkan_context,&mut system.renderer,extent)?);}else if !_resize{resize_transaction=world.stage_resize(&system.vulkan_context,&mut system.renderer,extent)?;}
            if !_resize{world.set_display_scale(system.egui_ctx.pixels_per_point());}
            // Pre-UI prepares layout only. Content ownership is coordinated
            // with the post-UI label candidate below.
            Ok(())
        })();
        if let Err(error)=result {
            // The collected overlay may still reference the prior target/scene.
            // Refuse submission of the entire candidate frame on World failure.
            system.annotation_prepare_error=Some(error.clone());self.gpu_error(&error);return;
        }
        // The first pass publishes only World layout required to collect UI.
        // Marking mesh/query buffers for the next serial here would prevent a
        // same-frame deletion from retiring them at the completed prior fence.
        if _resize{return;}
        system.annotation_prepare_error=None;
        if system.annotation_sample.is_some()||self.active_tool.distance_label().is_some()||system.annotation_ready_shape.is_some(){
            // Floating tool text is always visible and never samples depth.
            // Its publication must not depend on the independently admitted
            // source MSAA depth policy for scene dimensions and leaders.
            let include_annotations=system.annotation_sample.is_some();
            let sample=system.annotation_sample.unwrap_or(0);
            let result=(||{
                let factory=system.annotation_factory()?;
                let mut state=if system.annotation_font_refusal_for_probe{
                    annotation_viewport::ViewportAnnotations::with_font_limits(system.swapchain.stencil_enabled,ingetrazo_formats::annotation_font::Limits{faces:1,retained_font_bytes:1})?
                }else{match system.annotation_viewport.take(){Some(state)=>state,None=>annotation_viewport::ViewportAnnotations::new(system.swapchain.stencil_enabled)?}};
                let candidate=state.prepare(self,&factory,rect,system.egui_ctx.pixels_per_point(),include_annotations).map(|value|value.map(|value|(value.handle.clone(),value.queries.clone(),value.inverse,value.eye,value.forward)));
                if !system.annotation_font_refusal_for_probe{system.annotation_viewport=Some(state);}
                match candidate?{
                    Some((handle,queries,inverse,eye,forward))=>{
                        let renderer=system.annotation_cad.as_mut().ok_or("Annotation renderer absent")?;
                        let world=system.world.as_mut().ok_or("Annotation World absent")?;
                        world.prepare_annotation_overlay_transaction(renderer,&handle,&system.vulkan_context,queries,inverse,eye,forward,sample,system.fence,system.draw_serial,system.draw_serial+1,system.egui_ctx.pixels_per_point(),|world|prepare_app_world(self,world,&system.vulkan_context,system.fence,system.draw_serial))?;
                        system.annotation_ready_shape=Some(handle.shape(rect));
                    },
                    None=>{
                        // Clearing cannot require new font/CAD allocations.
                        // Keep old callback ownership until World succeeds.
                        let renderer=system.annotation_cad.as_mut().ok_or("Annotation renderer absent")?;
                        system.world.as_mut().ok_or("World absent")?.clear_annotation_overlay_transaction(renderer,&system.vulkan_context,system.fence,system.draw_serial,|world|prepare_app_world(self,world,&system.vulkan_context,system.fence,system.draw_serial))?;
                        system.annotation_ready_shape=None;
                    },
                }
                Ok::<(),String>(())
            })();
            system.annotation_prepare_error=result.as_ref().err().cloned();
            if let Err(error)=result{self.gpu_error(&error);}
        }else{
            let result=prepare_app_world(self,system.world.as_mut().expect("prepared World"),&system.vulkan_context,system.fence,system.draw_serial);
            system.annotation_prepare_error=result.as_ref().err().cloned();
            if let Err(error)=result{self.gpu_error(&error);}
        }
        if let Err(error)=system.world.as_mut().unwrap().finish_resize(&mut system.renderer,resize_transaction,system.annotation_prepare_error.is_none()){system.annotation_prepare_error=Some(error.clone());self.gpu_error(&error);}
    }
    fn build_ui(&mut self,ctx:&Context) { self.update_ui(ctx); }
    fn gpu_error(&mut self,error:&str) {self.status_message=format!("GPU rendering paused: {error}");eprintln!("{}",self.status_message);}
}

fn prepare_app_world(app:&crate::IngeTrazoApp,world:&mut world::World,context:&VulkanContext,fence:vk::Fence,serial:u64)->Result<(),String>{
 let sun=crate::solar_controls::scene_direction(app).unwrap_or(None);
 let sweep=world.prepare_face_sweep(context,&app.scene,&app.camera,&app.style,app.active_tool.face_sweep_preview(),app.model_revision,fence,serial)?;
 let result=if sun.is_some()||world.has_shadow_owner(){
  let ground=world::World::solar_ground_receiver_for_scene(&app.scene);
  world.set_scene_with_sun(context,&app.scene,&app.camera,&app.style,fence,serial,app.native_geometry_revision,sun,app.scene.shadows.darkness as f32,ground)
 }else if app.active_tool.face_sweep_preview().is_some(){world.set_scene_with_source_camera(context,&app.scene,&app.camera,&app.style,fence,serial,app.native_geometry_revision)}else{world.set_scene(context,&app.scene,&app.camera,&app.style,fence,serial,app.native_geometry_revision)};
 if let Err(error)=result{world.abort_face_sweep(sweep,fence,serial)?;return Err(error);}
 world.publish_face_sweep(sweep,&app.scene,serial);Ok(())
}

impl VulkanContext {
    /// Timestamp precision for the actual selected graphics family; no inferred GPU timing.
    pub fn graphics_timestamp_info(&self)->Option<(u32,f32)> {
        let queues=unsafe{self.instance.get_physical_device_queue_family_properties(self.physical_device)};
        let bits=queues[self.graphics_q_index as usize].timestamp_valid_bits;
        let period=unsafe{self.instance.get_physical_device_properties(self.physical_device)}.limits.timestamp_period;
        (bits!=0&&period>0.).then_some((bits,period))
    }
}

// Presentation keeps source encoded RGB blending in the attachment itself;
// an SRGB attachment would silently perform a different linear-light blend.
fn select_encoded_surface_format(formats:&[vk::SurfaceFormatKHR])->Option<vk::SurfaceFormatKHR>{
 for preferred in [vk::Format::B8G8R8A8_UNORM,vk::Format::R8G8B8A8_UNORM] {
  if let Some(format)=formats.iter().find(|f|f.format==preferred&&f.color_space==vk::ColorSpaceKHR::SRGB_NONLINEAR){return Some(*format);}
 }
 formats.iter().find(|f|f.format==vk::Format::UNDEFINED&&f.color_space==vk::ColorSpaceKHR::SRGB_NONLINEAR).map(|_|vk::SurfaceFormatKHR{format:vk::Format::B8G8R8A8_UNORM,color_space:vk::ColorSpaceKHR::SRGB_NONLINEAR})
}
#[cfg(test)]mod encoded_surface_tests {
 use super::*;
 #[test]fn prefers_direct_encoded_attachment_and_refuses_semantic_fallback(){
  let srgb=vk::SurfaceFormatKHR{format:vk::Format::B8G8R8A8_SRGB,color_space:vk::ColorSpaceKHR::SRGB_NONLINEAR};
  let unorm=vk::SurfaceFormatKHR{format:vk::Format::R8G8B8A8_UNORM,..srgb};
  assert_eq!(select_encoded_surface_format(&[srgb,unorm]),Some(unorm));assert_eq!(select_encoded_surface_format(&[srgb]),None);
  assert_eq!(select_encoded_surface_format(&[vk::SurfaceFormatKHR{format:vk::Format::UNDEFINED,..srgb}]).unwrap().format,vk::Format::B8G8R8A8_UNORM);
 }
}

fn cad_attachment_usage(supported:vk::ImageUsageFlags,integer:bool)->Result<vk::ImageUsageFlags,String>{
 let usage=vk::ImageUsageFlags::COLOR_ATTACHMENT|if integer{vk::ImageUsageFlags::INPUT_ATTACHMENT}else{vk::ImageUsageFlags::empty()};
 if !supported.contains(usage){return Err("Swapchain lacks INPUT_ATTACHMENT usage required for exact integer CAD2D composition".into());}Ok(usage)
}
#[cfg(test)]mod cad_input_usage_tests{use super::*;
 #[test]fn unsupported_input_is_explicit_refusal(){let color=vk::ImageUsageFlags::COLOR_ATTACHMENT;assert_eq!(cad_attachment_usage(color,false).unwrap(),color);assert!(cad_attachment_usage(color,true).unwrap_err().contains("INPUT_ATTACHMENT"));assert_eq!(cad_attachment_usage(color|vk::ImageUsageFlags::INPUT_ATTACHMENT,true).unwrap(),color|vk::ImageUsageFlags::INPUT_ATTACHMENT);assert!(cad_attachment_usage(vk::ImageUsageFlags::INPUT_ATTACHMENT,true).is_err());}
}
pub mod textures;
mod texture_average;

pub mod reference_images;

#[cfg(test)]mod annotation_slot_tests{
 use super::*;

 #[test]fn refused_frame_keeps_new_font_and_texture_updates_for_retry(){
  let mut pending=std::collections::VecDeque::new();let mut free=None;
  let delta=||egui::epaint::ImageDelta::full(egui::ColorImage::new([1,1],egui::Color32::RED),egui::TextureOptions::NEAREST);
  retain_frame_texture_deltas(&mut pending,&mut free,vec![(egui::TextureId::Managed(0),delta()),(egui::TextureId::Managed(7),delta())],&[egui::TextureId::Managed(9)]);
  // No GPU submission occurs. A second ctx.run cannot recreate the lost full atlas.
  retain_frame_texture_deltas(&mut pending,&mut free,vec![],&[egui::TextureId::Managed(10)]);
  assert_eq!(pending.len(),2);assert_eq!(pending[0].0,egui::TextureId::Managed(0));assert_eq!(pending[1].0,egui::TextureId::Managed(7));assert_eq!(free.unwrap(),vec![egui::TextureId::Managed(9),egui::TextureId::Managed(10)]);
 }
 #[test]fn current_transaction_resolves_first_add_delete_and_replacement_at_original_ui_order(){
  let rect=egui::Rect::from_min_size(egui::pos2(20.,30.),egui::vec2(100.,80.));
  let slot=||{let egui::Shape::Callback(mut value)=annotation_slot()else{unreachable!()};value.rect=rect;egui::epaint::ClippedShape{clip_rect:rect,shape:egui::Shape::Callback(value)}};
  let callback=|id:u32|egui::Shape::Callback(egui::PaintCallback{rect:egui::Rect::NOTHING,callback:std::sync::Arc::new(id)});
  let mut shapes=vec![slot()];resolve_annotation_slots(&mut shapes,Some(&callback(1)));
  let egui::Shape::Callback(value)=&shapes[0].shape else{panic!("first addition omitted")};assert_eq!(value.rect,rect);assert_eq!(value.callback.downcast_ref::<u32>(),Some(&1));
  let mut shapes=vec![slot()];resolve_annotation_slots(&mut shapes,None);assert!(matches!(shapes[0].shape,egui::Shape::Noop),"collected old overlay survived deletion");
  let mut shapes=vec![slot()];resolve_annotation_slots(&mut shapes,Some(&callback(2)));let egui::Shape::Callback(value)=&shapes[0].shape else{panic!()};assert_eq!(value.callback.downcast_ref::<u32>(),Some(&2));
 }
}
