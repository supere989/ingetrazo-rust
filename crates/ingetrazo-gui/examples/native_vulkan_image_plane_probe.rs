//! Render original image-plane IGZ through the production shared World/cache path.
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{
    budget::VulkanBudget, headless::DeviceRequest, world::World, VulkanContext,
};
use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let root = std::path::PathBuf::from(&args[1]);
    let out = std::path::PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out)?;
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("original-alpha-results.json"))?)?;
    let context =
        VulkanContext::new_headless(&if args.iter().any(|a| a == "--software-fixture") {
            DeviceRequest::software_fixture()
        } else {
            DeviceRequest::nvidia_hardware()
        })?;
    let properties = unsafe { context.instance.get_physical_device_properties(context.physical_device) };
    let device_name = unsafe { std::ffi::CStr::from_ptr(properties.device_name.as_ptr()) }.to_string_lossy().into_owned();
    let renderer_device = serde_json::json!({"name":device_name,"vendor_id":properties.vendor_id,"device_id":properties.device_id,"device_type":format!("{:?}",properties.device_type),"software_fixture_requested":args.iter().any(|a|a=="--software-fixture")});
    let budget = Arc::new(VulkanBudget::new(
        &context.instance,
        context.physical_device,
    )?);
    budget.ledger.lock().unwrap().total_limit = Some(128 * 1024 * 1024);
    let fence = unsafe {
        context.device.create_fence(
            &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
            None,
        )
    }?;
    let commands = unsafe {
        context.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(context.command_pool)
                .command_buffer_count(1),
        )
    }?;
    let mut rows = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        let mut scene = ingetrazo_formats::igz::load_igz(&root.join(format!("{name}.igz")))?;
        let mut camera = ingetrazo_core::camera::OrbitCamera::default();
        camera.set_standard_view(if case["camera_view"].as_str()==Some("iso"){ingetrazo_core::camera::StandardView::Iso}else{ingetrazo_core::camera::StandardView::Top});
        camera.target = DVec3::ZERO;
        camera.distance = 4.;
        camera.perspective = case["camera_perspective"].as_bool().unwrap_or(false);
        camera.aspect = 800. / 600.;
        let mut style = ingetrazo_render::style::StyleConfig::default();
        for i in 0..3 {
            style.background_color[i] = case["background"][i].as_f64().unwrap() as f32;
        }
        if let Some(mode)=case["style_mode"].as_str() {style.visual_style=match mode {"wireframe"=>ingetrazo_render::style::VisualStyle::Wireframe,"hidden_line"=>ingetrazo_render::style::VisualStyle::HiddenLine,"monochrome"=>ingetrazo_render::style::VisualStyle::Monochrome,"xray"=>ingetrazo_render::style::VisualStyle::XRay,_=>ingetrazo_render::style::VisualStyle::ShadedWithTextures};}
        if name.starts_with("style-") {for i in 0..3 {style.front_color[i]=case["style_front_color"][i].as_f64().unwrap()as f32;style.back_color[i]=case["style_back_color"][i].as_f64().unwrap()as f32;}}
        style.show_textures=case["show_textures"].as_bool().unwrap_or(true);
        style.show_sky = false;
        style.show_edges = case["show_edges"].as_bool().unwrap_or(false);
        style.show_profiles = false;
        let mut world = World::new(&context, budget.clone())?;
        world.resize_offscreen(
            &context,
            vk::Extent2D {
                width: 800,
                height: 600,
            },
        )?;
        world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
        world.set_scene(&context, &scene, &camera, &style, fence, 0, 1)?;
        assert_eq!(world.uploads, 1);
        let mut warm_pose_metrics=serde_json::Value::Null;
        if !scene.image_planes.is_empty(){
            let frame=[scene.image_planes[0].origin,scene.image_planes[0].u,scene.image_planes[0].v];
            let uploads=world.texture_stats().1.uploads;let geometry=world.uploads;let hashes=world.texture_stats().1.hashes;let decodes=world.texture_stats().1.decodes;
            for i in 1..11{scene.image_planes.pose_mut(0).unwrap().set_origin(frame[0]+DVec3::X*(i as f64/10.));world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;}
            scene.image_planes.pose_mut(0).unwrap().set_frame(frame);world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;
            assert_eq!(world.uploads,geometry,"pose-only image update rebuilt geometry");assert_eq!(world.texture_stats().1.uploads,uploads,"pose-only image update uploaded image");warm_pose_metrics=serde_json::json!({"pose_updates":11,"geometry_uploads":world.uploads-geometry,"image_uploads":world.texture_stats().1.uploads-uploads,"hashes":world.texture_stats().1.hashes-hashes,"decodes":world.texture_stats().1.decodes-decodes});
        }

        if name.starts_with("style-") {
            let committed=budget.ledger.lock().unwrap().total_committed();
            for mode in [ingetrazo_render::style::VisualStyle::Wireframe,ingetrazo_render::style::VisualStyle::XRay,ingetrazo_render::style::VisualStyle::HiddenLine,ingetrazo_render::style::VisualStyle::Monochrome] {
                let mut changed=style.clone();changed.visual_style=mode;changed.front_color=[0.3,0.2,0.1,1.];changed.back_color=[0.1,0.2,0.3,1.];
                world.set_scene(&context,&scene,&camera,&changed,fence,0,1)?;
                assert_eq!(world.uploads,1,"style-only change must reuse cached geometry");if !scene.show_section_cuts {assert_eq!(budget.ledger.lock().unwrap().total_committed(),committed);} // Cap visibility has separate legitimate allocation.
            }
            world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;
        }
        unsafe {
            context
                .device
                .reset_command_buffer(commands[0], vk::CommandBufferResetFlags::empty())?;
            context
                .device
                .begin_command_buffer(commands[0], &vk::CommandBufferBeginInfo::default())?;
        }
        world.record(commands[0], 1)?;
        unsafe {
            context.device.end_command_buffer(commands[0])?;
            context.device.reset_fences(&[fence])?;
            context.device.queue_submit(
                context.graphics_queue,
                &[vk::SubmitInfo::default().command_buffers(&commands)],
                fence,
            )?;
        }
        context.wait_transfer(fence)?;
        let image = world.readback(&context)?;
        image::save_buffer(
            out.join(format!("{name}.png")),
            &image,
            800,
            600,
            image::ColorType::Rgba8,
        )?;
        let before=world.texture_stats();let warm=(before.0.uploads,before.1.hashes,before.1.decodes,before.1.uploads);
        world.set_scene(&context,&scene,&camera,&style,fence,1,1)?;
        let after=world.texture_stats();assert_eq!(warm,(after.0.uploads,after.1.hashes,after.1.decodes,after.1.uploads),"Warm scene reuses actual sampled images and geometry");
        if name=="image-quadrants" {
            // Original XRay samples existing opaque texture runs, even when
            // show_textures is false. Presentation must not rehash/reupload.
            let world_uploads=world.uploads;
            let mut xray=style.clone();xray.visual_style=ingetrazo_render::style::VisualStyle::XRay;xray.show_textures=false;
            world.set_scene(&context,&scene,&camera,&xray,fence,1,1)?;
            let stats=world.texture_stats();assert_eq!(warm,(stats.0.uploads,stats.1.hashes,stats.1.decodes,stats.1.uploads));assert_eq!(world.uploads,world_uploads);
            world.set_scene(&context,&scene,&camera,&style,fence,1,1)?;
            let old=budget.ledger.lock().unwrap().total_committed();
            for bytes in scene.embedded_textures.values_mut(){bytes.push(0);}
            scene.version+=1;
            unsafe{context.device.reset_fences(&[fence])?;}
            let refused=world.set_scene(&context,&scene,&camera,&style,fence,1,1).expect_err("Incomplete draw fence must refuse resource replacement");assert!(refused.contains("completed draw fence"));assert_eq!(budget.ledger.lock().unwrap().total_committed(),old);assert_eq!(world.texture_stats().1.hashes,warm.1,"Fence refusal precedes decode/hash/upload");
            unsafe{context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default()],fence)?;}context.wait_transfer(fence)?;
            world.set_texture_pressure_for_probe(true);
            let error=world.set_scene(&context,&scene,&camera,&style,fence,1,1).expect_err("Real allocation pressure must refuse replacement");
            world.set_texture_pressure_for_probe(false);assert!(error.contains("Reference image admission"),"Pressure must reach actual image admission: {error}");
            assert_eq!(budget.ledger.lock().unwrap().total_committed(),old,"Failed replacement must preserve old cached allocation ledger");
            unsafe {context.device.reset_command_buffer(commands[0],vk::CommandBufferResetFlags::empty())?;context.device.begin_command_buffer(commands[0],&vk::CommandBufferBeginInfo::default())?;}
            world.record(commands[0],2)?;
            unsafe {context.device.end_command_buffer(commands[0])?;context.device.reset_fences(&[fence])?;context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&commands)],fence)?;}
            context.wait_transfer(fence)?;let restored=world.readback(&context)?;assert!(restored==image,"Actual sampled draw after failed image replacement must preserve every old pixel");
            world.set_scene(&context,&scene,&camera,&style,fence,2,1)?;assert_eq!(world.texture_stats().1.retired,1,"Old image retires only after its sampled draw fence completes");
            eprintln!("Texture replacement pressure refused: {error}; old sampled image and ledger preserved; replacement retry and fenced image retirement passed");
        }
        let samples=case["points"].as_array().unwrap().iter().map(|p|{let x=p[0].as_u64().unwrap()as usize;let y=p[1].as_u64().unwrap()as usize;let i=(y*800+x)*4;image[i..i+4].to_vec()}).collect::<Vec<_>>();
        let offset = (300 * 800 + 400) * 4;
        rows.push(serde_json::json!({"case":name,"native_mvp":(camera.projection_matrix()*camera.view_matrix()).as_mat4().to_cols_array(),"native_projection":camera.projection_matrix().as_mat4().to_cols_array(),"native_view":camera.view_matrix().as_mat4().to_cols_array(),"center_rgba":&image[offset..offset+4],"samples":samples,"texture_stats":format!("{:?}",world.texture_stats()),"warm_pose_metrics":warm_pose_metrics,"reference_image_stats":format!("{:?}",world.reference_image_stats()),"geometry_uploads":world.uploads,"hard_edge_bytes":world.edge_geometry_bytes(),"hard_edge_endpoint_count":world.edge_endpoint_count(),"opacity_runs":world.transparent_runs(),"paint_runs":world.paint_runs(),"ordinary_runs":world.ordinary_paint_runs(),"consolidated_opaque_count":world.consolidated_opaque_count(),"surface_bytes":world.transparent_surface_bytes(),"contour_metrics":format!("{:?}",world.contour_metrics())}));
        drop(world);
        assert_eq!(budget.ledger.lock().unwrap().total_committed(), 0);
    }
    unsafe {
        context
            .device
            .free_command_buffers(context.command_pool, &commands);
        context.device.destroy_fence(fence, None);
    }
    let result = serde_json::json!({"renderer_device":renderer_device,"cases":rows,"scope":"Production World native GPU input/reference corpus render; no parity assertion in renderer fixture itself"});
    std::fs::write(
        out.join("native-alpha-results.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
