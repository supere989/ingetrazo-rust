pub mod cad2d;
pub mod headless;
pub mod world;
pub mod resources;
pub mod texture_cache;
mod queues;
pub mod budget;
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
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

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
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}

struct AppWrapper<A: App> {
    title: &'static str,
    proxy: winit::event_loop::EventLoopProxy<()>,
    window: Option<Window>,
    app: Option<System>,
    egui_app: Option<A>,
    initial_file: Option<std::path::PathBuf>,
}

impl<A: App> ApplicationHandler for AppWrapper<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = create_window(self.title, event_loop).unwrap();
        let mut app = System::new(self.title, &window).unwrap();
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

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.app
            .as_mut()
            .unwrap()
            .on_exit(self.egui_app.as_mut().unwrap());
    }
}

pub struct System {
    command_buffer: vk::CommandBuffer,
    swapchain: Swapchain,
    image_available_semaphore: vk::Semaphore,
    // Presentation may outlive the graphics fence; one semaphore per acquired image.
    render_finished_semaphores: Vec<vk::Semaphore>,
    fence: vk::Fence,

    run: bool,
    dirty_swapchain: bool,

    pub egui_ctx: Context,
    pub egui_winit: State,
    pub renderer: Renderer,
    pub memory_budget: std::sync::Arc<budget::VulkanBudget>,
    pub world: Option<world::World>,
    pub cad2d: Option<cad2d::Renderer>,
    draw_serial:u64,

    textures_to_free: Option<Vec<TextureId>>,
    close_requested: bool,
    repaint_at: std::time::Instant,
    pending_texture_updates: std::collections::VecDeque<(TextureId,egui::epaint::ImageDelta)>,
    last_gpu_error: Option<String>,

    pub vulkan_context: VulkanContext,
}

impl System {
    pub fn new(title: &str, window: &Window) -> Result<Self, Box<dyn Error>> {
        log::info!("Create application");

        let vulkan_context = VulkanContext::new(&window, title)?;

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

        let swapchain = Swapchain::new(&vulkan_context)?;

        // Semaphore use for presentation
        let image_available_semaphore = {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            unsafe {
                vulkan_context
                    .device
                    .create_semaphore(&semaphore_info, None)?
            }
        };
        let render_finished_semaphores = create_present_semaphores(&vulkan_context.device,swapchain.images.len())?;
        let fence = {
            let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            unsafe { vulkan_context.device.create_fence(&fence_info, None)? }
        };

        let egui_ctx = Context::default();

        let egui_winit = State::new(
            egui_ctx.clone(),
            ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        );

        let memory_budget=std::sync::Arc::new(budget::VulkanBudget::new(&vulkan_context.instance,vulkan_context.physical_device)?);
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

        Ok(Self {
            vulkan_context,
            command_buffer,
            swapchain,
            image_available_semaphore,
            render_finished_semaphores,
            fence,

            run: true,
            dirty_swapchain: false,

            egui_ctx,
            egui_winit,
            renderer,
            memory_budget,
            world:None,cad2d:None,draw_serial:0,
            textures_to_free: None,
            close_requested: false,
            repaint_at: std::time::Instant::now(),
            pending_texture_updates:std::collections::VecDeque::new(),
            last_gpu_error:None,
        })
    }

    pub fn enable_cad2d(&mut self)->Result<&cad2d::Renderer,String>{
        if self.cad2d.is_none(){self.cad2d=Some(cad2d::Renderer::new(&self.vulkan_context,self.memory_budget.clone(),self.swapchain.render_pass)?);}
        Ok(self.cad2d.as_ref().unwrap())
    }

    pub fn verify_cad2d_rollback_for_probe(&mut self)->Result<(),String>{
        self.enable_cad2d()?;
        self.cad2d.as_mut().unwrap().verify_pressure_rollback_for_probe(&self.vulkan_context,self.fence,self.draw_serial)
    }

    fn draw<A: App>(&mut self, egui_app: &mut A, window: &Window) {
        // If swapchain must be recreated wait for windows to not be minimized anymore
        if self.dirty_swapchain {
            let PhysicalSize { width, height } = window.inner_size();
            if width > 0 && height > 0 {
                self.swapchain
                    .recreate(&self.vulkan_context)
                    .expect("Failed to recreate swapchain");
                // recreate waits for device idle, including pending presentation use.
                unsafe { for semaphore in self.render_finished_semaphores.drain(..) { self.vulkan_context.device.destroy_semaphore(semaphore,None); } }
                self.render_finished_semaphores=create_present_semaphores(&self.vulkan_context.device,self.swapchain.images.len()).expect("Failed to create presentation semaphores");
                self.renderer
                    .set_render_pass(self.swapchain.render_pass)
                    .expect("Failed to rebuild renderer pipeline");
                if let Some(cad)=&mut self.cad2d{
                    if let Err(error)=cad.set_render_pass(self.swapchain.render_pass){egui_app.gpu_error(&error);self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);return;}
                }
                self.dirty_swapchain = false;
            } else {
                return;
            }
        }

        if !self.run {
            return;
        }

        unsafe {
            self.vulkan_context
                .device
                .wait_for_fences(&[self.fence], true, std::u64::MAX)
                .expect("Failed to wait ")
        };

        // Free last frames textures after the previous frame is done rendering
        if let Some(textures) = self.textures_to_free.take() {
            self.renderer
                .free_textures(&textures)
                .expect("Failed to free textures");
        }

        egui_app.prepare_gpu(self,true);

        // Generate UI
        let mut raw_input = self.egui_winit.take_egui_input(&window);
        if self.close_requested { raw_input.viewports.entry(ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close); }

        let egui::FullOutput {
            platform_output,
            textures_delta,
            shapes,
            pixels_per_point,
            viewport_output,
        } = self.egui_ctx.run(raw_input, |ctx| {
            egui_app.build_ui(ctx);
        });

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


        if !textures_delta.free.is_empty() {
            self.textures_to_free = Some(textures_delta.free.clone());
        }

        self.pending_texture_updates.extend(textures_delta.set);
        while let Some(delta)=self.pending_texture_updates.front() {
            if let Err(error)=self.renderer.set_textures(self.vulkan_context.graphics_queue,self.vulkan_context.command_pool,std::slice::from_ref(delta)) {
                let error=error.to_string();
                if self.last_gpu_error.as_ref()!=Some(&error) {egui_app.gpu_error(&error);self.last_gpu_error=Some(error);}
                self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);
                return; // Preserve document and last presented frame; pending deltas retry after pressure falls.
            }
            self.pending_texture_updates.pop_front();self.last_gpu_error=None;
        }

        let clipped_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);

        if let Some(cad)=&mut self.cad2d {
            if let Err(error)=cad.prepare(&self.vulkan_context,&clipped_primitives,self.fence,self.draw_serial,self.draw_serial+1){
                egui_app.gpu_error(&error);
                self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);
                // Old ready paint remains valid; continue presentation under pressure.
            }
        }

        // Drawing the frame
        let next_image_result = unsafe {
            self.swapchain.loader.acquire_next_image(
                self.swapchain.khr,
                std::u64::MAX,
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
            Err(error) => panic!("Error while acquiring next image. Cause: {}", error),
        };

        unsafe {
            self.vulkan_context
                .device
                .reset_fences(&[self.fence])
                .expect("Failed to reset fences")
        };

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let wait_semaphores = [self.image_available_semaphore];
        let signal_semaphores = [self.render_finished_semaphores[image_index as usize]];

        // Re-record commands to draw geometry
        let draw_error=record_command_buffers(
            &self.vulkan_context.device,
            self.vulkan_context.command_pool,
            self.command_buffer,
            self.swapchain.framebuffers[image_index as usize],
            self.swapchain.render_pass,
            self.swapchain.extent,
            pixels_per_point,
            &mut self.renderer,
            &clipped_primitives,
            self.world.as_mut(),self.draw_serial+1,
        )
        .expect("Failed to record command buffer");
        if let Some(error)=draw_error {egui_app.gpu_error(&error);self.repaint_at=std::time::Instant::now()+std::time::Duration::from_secs(1);}

        let command_buffers = [self.command_buffer];
        let submit_info = [vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)];
        unsafe {
            self.vulkan_context
                .device
                .queue_submit(self.vulkan_context.graphics_queue, &submit_info, self.fence)
                .expect("Failed to submit work to gpu.")
        };

        self.draw_serial+=1;
        let swapchains = [self.swapchain.khr];
        let images_indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&images_indices);

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
            Err(error) => panic!("Failed to present queue. Cause: {}", error),
            _ => {}
        }
    }

    fn on_window_event(
        &mut self,
        event: &WindowEvent,
        window: &Window,
        _event_loop: &ActiveEventLoop,
    ) {
        let response = self.egui_winit.on_window_event(&window, &event);
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

    fn on_exit<A: App>(&mut self, egui_app: &mut A) {
        log::info!("Stopping application");
        eprintln!("{}",self.memory_budget.diagnostics());

        unsafe {
            self.vulkan_context
                .device
                .device_wait_idle()
                .expect("Failed to wait for graphics device to idle.");

            egui_app.clean(&self.vulkan_context);
            if let Some(world)=&self.world {eprintln!("Native world GPU surface uploads: {}",world.uploads);}
            drop(self.world.take());
            if let Some(cad)=&self.cad2d{eprintln!("CAD2D metrics: {:?}",cad.stats());}
            drop(self.cad2d.take());

            self.vulkan_context.device.destroy_fence(self.fence, None);
            self.vulkan_context
                .device
                .destroy_semaphore(self.image_available_semaphore, None);
            for semaphore in self.render_finished_semaphores.drain(..) { self.vulkan_context.device.destroy_semaphore(semaphore,None); }

            self.swapchain.destroy(&self.vulkan_context);

            self.vulkan_context
                .device
                .free_command_buffers(self.vulkan_context.command_pool, &[self.command_buffer]);
        }
    }
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
    _entry: Entry,
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
    pub transfer_timeout_ns:u64,
    pub diagnostic_fail_fast:bool,
}

impl VulkanContext {
    pub fn wait_transfer(&self,fence:vk::Fence)->Result<(),vk::Result> {
        let result=unsafe {self.device.wait_for_fences(&[fence],true,self.transfer_timeout_ns)};
        if result==Err(vk::Result::TIMEOUT)&&self.diagnostic_fail_fast {
            eprintln!("GPU diagnostic fence exceeded {}ns; exiting without freeing potentially pending resources",self.transfer_timeout_ns);
            std::process::exit(124);
        }
        result
    }
    pub fn new(window: &Window, name: &str) -> Result<Self, Box<dyn Error>> {
        // Vulkan instance
        let entry = unsafe { Entry::load()? };
        let (instance, debug_utils, debug_utils_messenger) =
            create_vulkan_instance(&entry, window, name)?;

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

        // Vulkan physical device and queue families indices (graphics and present)
        let (physical_device, graphics_q_index, present_q_index) =
            create_vulkan_physical_device_and_get_graphics_and_present_qs_indices(
                &instance,
                &surface,
                surface_khr,
            )?;

        // Vulkan logical device and queues
        let (device, graphics_queue, present_queue) =
            create_vulkan_device_and_graphics_and_present_qs(
                &instance,
                physical_device,
                graphics_q_index,
                present_q_index,
            )?;

        // Command pool & buffer
        let command_pool = {
            let command_pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_q_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            unsafe { device.create_command_pool(&command_pool_info, None)? }
        };

        Ok(Self {
            _entry: entry,
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
            transfer_timeout_ns:u64::MAX,
            diagnostic_fail_fast:false,
        })
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        log::debug!("Destroying Vulkan Context");
        unsafe {
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            if self.surface_khr!=vk::SurfaceKHR::null(){self.surface.destroy_surface(self.surface_khr, None);}
            if self.debug_utils_messenger!=vk::DebugUtilsMessengerEXT::null(){self.debug_utils.destroy_debug_utils_messenger(self.debug_utils_messenger, None);}
            self.instance.destroy_instance(None);
        }
    }
}

struct Swapchain {
    loader: swapchain::Device,
    extent: vk::Extent2D,
    khr: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    srgb: bool,
    image_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
}

impl Swapchain {
    fn new(vulkan_context: &VulkanContext) -> Result<Self, Box<dyn Error>> {
        // Swapchain
        let (loader, khr, extent, format, images, image_views) =
            create_vulkan_swapchain(&vulkan_context)?;

        // Renderpass
        let render_pass = create_vulkan_render_pass(&vulkan_context.device, format)?;

        // Framebuffers
        let framebuffers =
            create_vulkan_framebuffers(&vulkan_context.device, render_pass, extent, &image_views)?;

        Ok(Self {
            loader,
            extent,
            khr,
            images,
            srgb: matches!(format,vk::Format::R8G8B8A8_SRGB|vk::Format::B8G8R8A8_SRGB),
            image_views,
            render_pass,
            framebuffers,
        })
    }

    fn recreate(&mut self, vulkan_context: &VulkanContext) -> Result<(), Box<dyn Error>> {
        log::debug!("Recreating the swapchain");

        unsafe { vulkan_context.device.device_wait_idle()? };

        self.destroy(vulkan_context);

        // Swapchain
        let (loader, khr, extent, format, images, image_views) =
            create_vulkan_swapchain(vulkan_context)?;

        // Renderpass
        let render_pass = create_vulkan_render_pass(&vulkan_context.device, format)?;

        // Framebuffers
        let framebuffers =
            create_vulkan_framebuffers(&vulkan_context.device, render_pass, extent, &image_views)?;

        self.loader = loader;
        self.extent = extent;
        self.khr = khr;
        self.images = images;
        let srgb=matches!(format,vk::Format::R8G8B8A8_SRGB|vk::Format::B8G8R8A8_SRGB);
        if srgb!=self.srgb { return Err("Swapchain framebuffer color space changed; renderer recreation required".into()); }
        self.image_views = image_views;
        self.render_pass = render_pass;
        self.framebuffers = framebuffers;

        Ok(())
    }

    fn destroy(&mut self, vulkan_context: &VulkanContext) {
        unsafe {
            self.framebuffers
                .iter()
                .for_each(|fb| vulkan_context.device.destroy_framebuffer(*fb, None));
            self.framebuffers.clear();
            vulkan_context
                .device
                .destroy_render_pass(self.render_pass, None);
            self.image_views
                .iter()
                .for_each(|v| vulkan_context.device.destroy_image_view(*v, None));
            self.image_views.clear();
            self.loader.destroy_swapchain(self.khr, None);
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
    let debug_utils_messenger =
        unsafe { debug_utils.create_debug_utils_messenger(&create_info, None)? };

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
) -> Result<(Device, vk::Queue, vk::Queue), Box<dyn Error>> {
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

    let device_extensions_ptrs = [
        swapchain::NAME.as_ptr(),
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        ash::khr::portability_subset::NAME.as_ptr(),
    ];

    let available=unsafe{instance.get_physical_device_features(physical_device)};
    let features=vk::PhysicalDeviceFeatures::default().shader_clip_distance(available.shader_clip_distance!=0);
    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_create_infos)
        .enabled_extension_names(&device_extensions_ptrs)
        .enabled_features(&features);

    let device = unsafe { instance.create_device(physical_device, &device_create_info, None)? };
    let graphics_queue = unsafe { device.get_device_queue(graphics_q_index, 0) };
    let present_queue = unsafe { device.get_device_queue(present_q_index, 0) };

    Ok((device, graphics_queue, present_queue))
}

fn create_vulkan_swapchain(
    vulkan_context: &VulkanContext,
) -> Result<
    (
        swapchain::Device,
        vk::SwapchainKHR,
        vk::Extent2D,
        vk::Format,
        Vec<vk::Image>,
        Vec<vk::ImageView>,
    ),
    Box<dyn Error>,
> {
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
            .surface(vulkan_context.surface_khr)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT);

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
    let swapchain_khr = unsafe { swapchain.create_swapchain(&create_info, None)? };

    // Swapchain images and image views
    let images = unsafe { swapchain.get_swapchain_images(swapchain_khr)? };
    let views = images
        .iter()
        .map(|image| {
            let create_info = vk::ImageViewCreateInfo::default()
                .image(*image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            unsafe { vulkan_context.device.create_image_view(&create_info, None) }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((
        swapchain,
        swapchain_khr,
        extent,
        format.format,
        images,
        views,
    ))
}

fn create_vulkan_render_pass(
    device: &Device,
    format: vk::Format,
) -> Result<vk::RenderPass, Box<dyn Error>> {
    log::debug!("Creating vulkan render pass");
    let attachment_descs = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

    let color_attachment_refs = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

    let subpass_descs = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs)];

    let subpass_deps = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )];

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
    image_views: &[vk::ImageView],
) -> Result<Vec<vk::Framebuffer>, Box<dyn Error>> {
    log::debug!("Creating vulkan framebuffers");
    Ok(image_views
        .iter()
        .map(|view| [*view])
        .map(|attachments| {
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1);
            unsafe { device.create_framebuffer(&framebuffer_info, None) }
        })
        .collect::<Result<Vec<_>, _>>()?)
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
    world:Option<&mut world::World>,serial:u64,
) -> Result<Option<String>, Box<dyn Error>> {
    unsafe { device.reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty())? };

    let command_buffer_begin_info =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::SIMULTANEOUS_USE);
    unsafe { device.begin_command_buffer(command_buffer, &command_buffer_begin_info)? };
    if let Some(world)=world {world.record(command_buffer,serial)?;}

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
        }]);

    unsafe {
        device.cmd_begin_render_pass(
            command_buffer,
            &render_pass_begin_info,
            vk::SubpassContents::INLINE,
        )
    };

    let draw_error=renderer.cmd_draw(command_buffer,extent,pixels_per_point,clipped_primitives).err().map(|e|e.to_string());
    if draw_error.is_some() {
        // Previous fence is complete and this command buffer has not submitted.
        // Abort all partial callback commands, then consume the acquired frame
        // through a clear-only pass. The host reports failure and retries.
        unsafe {
            device.reset_command_buffer(command_buffer,vk::CommandBufferResetFlags::empty())?;
            device.begin_command_buffer(command_buffer,&command_buffer_begin_info)?;
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
        match system.enable_cad2d(){Ok(renderer)=>app.native_composer.factory=Some(renderer.factory()),Err(error)=>app.gpu_error(&error)}
        app.native_gpu_viewport_enabled=std::env::var_os("INGETRAZO_VULKAN_WORLD").is_some();
        app.system_clipboard=arboard::Clipboard::new().ok();
        app.attach_session_store(crate::session_state::SessionStore::user_default());
        if let Some(path)=initial_file { app.open_document(&path); }
        app
    }
    fn prepare_gpu(&mut self,system:&mut System,_resize:bool) {
        if !self.native_gpu_viewport_enabled||self.scene.active_sheet_index.is_some(){return;}
        let Some(rect)=self.native_gpu_viewport_rect else {return;};
        let result=(|| {
            if system.world.is_none(){system.world=Some(world::World::new(&system.vulkan_context,system.memory_budget.clone())?);}
            let world=system.world.as_mut().unwrap();
            // Layout can change independently of OS resize; the current rect
            // is authoritative. World.resize returns the cached texture in O(1)
            // if extent is unchanged, with no allocation/upload/copy.
            let scale=system.egui_ctx.pixels_per_point();let extent=vk::Extent2D{width:(rect.width()*scale).ceil().max(1.0) as u32,height:(rect.height()*scale).ceil().max(1.0) as u32};self.native_gpu_viewport_texture=Some(world.resize(&system.vulkan_context,&mut system.renderer,extent)?);
            world.set_display_scale(system.egui_ctx.pixels_per_point());
            world.set_scene(&system.vulkan_context,&self.scene,&self.camera,&self.style,system.fence,system.draw_serial,self.model_revision)
        })();
        if let Err(error)=result {self.gpu_error(&error);}
    }
    fn build_ui(&mut self,ctx:&Context) { self.update_ui(ctx); }
    fn gpu_error(&mut self,error:&str) {self.status_message=format!("GPU rendering paused: {error}");eprintln!("{}",self.status_message);}
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

pub mod textures;
