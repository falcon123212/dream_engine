use winit::{
    event::{Event, WindowEvent, DeviceEvent, ElementState, MouseButton, MouseScrollDelta},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use dream_forge::{
    context::ForgeContext,
    renderer::ForgeRenderer,
    swapchain::ForgeSwapchain,
    pipeline::PipelineManager,
    shader_compiler::ShaderCompiler,
    memory::{StagingBelt, MegaBuffer},
};
use seed_architect::importer::{SeedImporter, MaterialData};
use seed_architect::SeedFileHeader;

use ash::vk;
use glam::{Mat4, Vec3, Vec4};
use shaderc::ShaderKind;
use std::fs::File;
use std::io::Read;
use log::info;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("LUCID RUNTIME | ACCUMULATION FIX")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
        .build(&event_loop).expect("❌ Fenêtre KO");

    // 1. Initialisation
    let forge = ForgeContext::init(&window, "LucidEngine");
    let renderer = ForgeRenderer::new(&forge);
    let mut swapchain = ForgeSwapchain::new(&forge, &window);
    let shader_compiler = ShaderCompiler::new();
    
    let mut yaw: f32 = -90.0;
    let mut pitch: f32 = 0.0;
    let mut distance: f32 = 5.0;
    let mut is_right_click = false;
    let mut mouse_pos = (0.0f64, 0.0f64);
    let mut frame_index: u32 = 0;

    let mut universe = Some(MegaBuffer::new(&forge.memory, 1024 * 1024 * 1024));
    let mut staging = Some(StagingBelt::new(&forge.memory, 256 * 1024 * 1024));

    // 2. Ingestion .SEED
    let seed_path = "assets/processed/relic.seed";
    if !std::path::Path::new(seed_path).exists() {
        SeedImporter::import_and_bake("assets/raw/a.obj", seed_path);
    }

    let mut file = File::open(seed_path).expect("❌ Fichier .SEED KO");
    let header_size = std::mem::size_of::<SeedFileHeader>() + 8; 
    let mut header_buf = vec![0u8; header_size];
    file.read_exact(&mut header_buf).ok();
    let header: SeedFileHeader = bincode::deserialize(&header_buf).expect("❌ Header KO");

    let metadata = file.metadata().unwrap();
    let remaining = metadata.len() - header_size as u64;
    let mut vertex_data = vec![0u8; remaining as usize];
    file.read_exact(&mut vertex_data).expect("❌ Data KO");

    let (geo_offset, geo_ptr) = universe.as_mut().unwrap().allocate::<u8>(vertex_data.len() as u64, 16);
    let (_mat_offset, mat_ptr) = universe.as_mut().unwrap().allocate::<MaterialData>(64, 16);
    let (res_offset, res_ptr) = universe.as_mut().unwrap().allocate::<u32>(16, 16);

    let default_mat = [MaterialData {
        base_color: [1.0, 0.84, 0.0],
        metallic: 1.0,
        roughness: 0.1,
        ior: 1.45,
        emissive_ptr: 0,
        _padding: [0; 2],
    }];

    unsafe {
        let cmd = renderer.command_buffer;
        let _ = forge.device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT));
        
        // FIX: On push le contenu de la slice (Vertex Data)
        let (s_buf, s_off) = staging.as_mut().unwrap().push(&vertex_data[..]);
        let copy_geo = vk::BufferCopy::builder().src_offset(s_off).dst_offset(geo_offset).size(vertex_data.len() as u64);
        forge.device.cmd_copy_buffer(cmd, s_buf, universe.as_ref().unwrap().buffer_handle(), &[copy_geo.build()]);

        // FIX: On push le contenu de la slice (Material) + Taille 40
        let (s_buf_m, s_off_m) = staging.as_mut().unwrap().push(&default_mat[..]);
        let copy_mat = vk::BufferCopy::builder().src_offset(s_off_m).dst_offset(_mat_offset).size(std::mem::size_of_val(&default_mat) as u64);
        forge.device.cmd_copy_buffer(cmd, s_buf_m, universe.as_ref().unwrap().buffer_handle(), &[copy_mat.build()]);

        let reset_res = [0u32, f32::MAX.to_bits()];
        let (s_buf_r, s_off_r) = staging.as_mut().unwrap().push(bytemuck::cast_slice::<u32, u8>(&reset_res));
        let copy_res = vk::BufferCopy::builder().src_offset(s_off_r).dst_offset(res_offset).size(8);
        forge.device.cmd_copy_buffer(cmd, s_buf_r, universe.as_ref().unwrap().buffer_handle(), &[copy_res.build()]);

        let _ = forge.device.end_command_buffer(cmd);
        forge.device.queue_submit(forge.queue, &[vk::SubmitInfo::builder().command_buffers(&[cmd]).build()], vk::Fence::null()).unwrap();
        forge.device.device_wait_idle().unwrap();
        staging.as_mut().unwrap().reset();
    }

    // 3. Pipeline
    let pipeline = PipelineManager::new(
        &forge,
        shader_compiler.compile_file(&forge.device, std::path::Path::new("assets/shaders/surface.glsl"), ShaderKind::Vertex).unwrap(),
        shader_compiler.compile_file(&forge.device, std::path::Path::new("assets/shaders/surface.glsl"), ShaderKind::Fragment).unwrap(),
        shader_compiler.compile_file(&forge.device, std::path::Path::new("assets/shaders/picker.comp"), ShaderKind::Compute).unwrap(),
        swapchain.format,
        swapchain.depth_format,
    );

    // 4. Descriptor Accumulation
    let descriptor_pool = unsafe {
        forge.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::builder()
            .max_sets(1).pool_sizes(&[vk::DescriptorPoolSize::builder().ty(vk::DescriptorType::STORAGE_IMAGE).descriptor_count(1).build()]), None).unwrap()
    };
    let accum_set = unsafe {
        forge.device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool).set_layouts(&[pipeline.descriptor_set_layout])).unwrap()[0]
    };
    unsafe {
        let img_info = [vk::DescriptorImageInfo::builder().image_view(swapchain.accum_view).image_layout(vk::ImageLayout::GENERAL).build()];
        forge.device.update_descriptor_sets(&[vk::WriteDescriptorSet::builder()
            .dst_set(accum_set).dst_binding(0).descriptor_type(vk::DescriptorType::STORAGE_IMAGE).image_info(&img_info).build()], &[]);
    }

    // -------------------------------

    // -------------------------------

    info!("🚀 Moteur prêt. {} atomes chargés.", header.vertex_count);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::MainEventsCleared => window.request_redraw(),
            
            Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => { mouse_pos = (position.x, position.y); }
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                if is_right_click {
                    yaw += delta.0 as f32 * 0.1;
                    pitch = (pitch - delta.1 as f32 * 0.1).clamp(-89.0, 89.0);
                    frame_index = 0;
                }
            }
            Event::WindowEvent { event: WindowEvent::MouseInput { state, button, .. }, .. } => {
                if button == MouseButton::Right { is_right_click = state == ElementState::Pressed; }
                
                if button == MouseButton::Left && state == ElementState::Pressed {
                    let eye = Vec3::new(distance * pitch.to_radians().cos() * yaw.to_radians().cos(), distance * pitch.to_radians().sin(), distance * pitch.to_radians().cos() * yaw.to_radians().sin());
                    let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
                    let proj = Mat4::perspective_rh(45.0f32.to_radians(), swapchain.extent.width as f32 / swapchain.extent.height as f32, 0.1, 1000.0);
                    let correction = Mat4::from_cols(Vec4::new(1.0, 0.0, 0.0, 0.0), Vec4::new(0.0, 1.0, 0.0, 0.0), Vec4::new(0.0, 0.0, 0.5, 0.0), Vec4::new(0.0, 0.0, 0.5, 1.0));
                    let inv_vp = (correction * proj * view).inverse();
                    
                    let nx = (2.0 * mouse_pos.0 as f32 / swapchain.extent.width as f32) - 1.0;
                    let ny = (2.0 * mouse_pos.1 as f32 / swapchain.extent.height as f32) - 1.0;
                    let near = inv_vp * Vec4::new(nx, ny, 0.0, 1.0);
                    let far = inv_vp * Vec4::new(nx, ny, 1.0, 1.0);
                    let ray_origin = near.truncate() / near.w;
                    let ray_dir = (far.truncate() / far.w - ray_origin).normalize();

                    let reset_data: [u32; 2] = [0u32, f32::MAX.to_bits()];
                    unsafe {
                        let cmd = renderer.command_buffer;
                        let _ = forge.device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT));
                        
                        let (s_buf, s_off) = staging.as_mut().unwrap().push(bytemuck::cast_slice::<u32, u8>(&reset_data));
                        forge.device.cmd_copy_buffer(cmd, s_buf, universe.as_ref().unwrap().buffer_handle(), &[vk::BufferCopy { src_offset: s_off, dst_offset: res_offset, size: 8 }]);
                        
                        let mut pc_compute = [0u8; 64];
                        pc_compute[0..8].copy_from_slice(&geo_ptr.device_address.to_ne_bytes());
                        pc_compute[8..16].copy_from_slice(&res_ptr.device_address.to_ne_bytes());
                        pc_compute[16..20].copy_from_slice(&(header.vertex_count as u32).to_ne_bytes());
                        pc_compute[32..44].copy_from_slice(bytemuck::cast_slice(&ray_origin.to_array()));
                        pc_compute[48..60].copy_from_slice(bytemuck::cast_slice(&ray_dir.to_array()));
                        
                        forge.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline.compute_pipeline);
                        forge.device.cmd_push_constants(cmd, pipeline.compute_layout, vk::ShaderStageFlags::COMPUTE, 0, &pc_compute);
                        forge.device.cmd_dispatch(cmd, (header.vertex_count as u32 + 255) / 256, 1, 1);
                        
                        let _ = forge.device.end_command_buffer(cmd);
                        forge.device.queue_submit(forge.queue, &[vk::SubmitInfo::builder().command_buffers(&[cmd]).build()], vk::Fence::null()).unwrap();
                        forge.device.device_wait_idle().unwrap();
                        staging.as_mut().unwrap().reset();
                        
                        let id = universe.as_ref().unwrap().read_value::<u32>(res_offset);
                        if id != 0 { println!("🎯 IMPACT ! Atome #{}", id); }
                    }
                }
            }
            Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } => {
                if let MouseScrollDelta::LineDelta(_, y) = delta { 
                    distance = (distance - y * 0.5).clamp(1.0, 50.0);
                    frame_index = 0; 
                }
            }

            Event::RedrawRequested(_) => {
                let img_idx = match swapchain.acquire_next_image(renderer.image_available_sem) {
                    Some(idx) => idx,
                    None => return,
                };
                renderer.begin_frame(&forge);

                unsafe {
                    let cmd = renderer.command_buffer;
                    let _ = forge.device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty());
                    let _ = forge.device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT));

                    // --- FIX TRANSITION SWAPCHAIN ---
                    let swapchain_barrier = vk::ImageMemoryBarrier::builder()
                        .image(swapchain.images[img_idx as usize])
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .subresource_range(vk::ImageSubresourceRange::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .level_count(1).layer_count(1).build())
                        .build();

                    forge.device.cmd_pipeline_barrier(cmd,
                        vk::PipelineStageFlags::TOP_OF_PIPE, // On attend rien
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, // On débloque l'écriture couleur
                        vk::DependencyFlags::empty(), &[], &[], &[swapchain_barrier]);
                    // --------------------------------

                    let color_att = vk::RenderingAttachmentInfo::builder()
                        .image_view(swapchain.image_views[img_idx as usize]).image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.01, 0.01, 0.01, 1.0] } }).build();
                    let depth_att = vk::RenderingAttachmentInfo::builder()
                        .image_view(swapchain.depth_view).image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } }).build();

                    forge.device.cmd_begin_rendering(cmd, &vk::RenderingInfo::builder().render_area(vk::Rect2D { extent: swapchain.extent, ..Default::default() }).layer_count(1).color_attachments(std::slice::from_ref(&color_att)).depth_attachment(&depth_att));

                    let eye = Vec3::new(distance * pitch.to_radians().cos() * yaw.to_radians().cos(), distance * pitch.to_radians().sin(), distance * pitch.to_radians().cos() * yaw.to_radians().sin());
                    let view = Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
                    let proj = Mat4::perspective_rh(45.0f32.to_radians(), swapchain.extent.width as f32 / swapchain.extent.height as f32, 0.1, 1000.0);
                    let correction = Mat4::from_cols(Vec4::new(1.0, 0.0, 0.0, 0.0), Vec4::new(0.0, 1.0, 0.0, 0.0), Vec4::new(0.0, 0.0, 0.5, 0.0), Vec4::new(0.0, 0.0, 0.5, 1.0));
                    let view_proj = correction * proj * view;

                    let mut push_data = [0u8; 176];
                    push_data[0..8].copy_from_slice(&geo_ptr.device_address.to_ne_bytes());
                    push_data[8..16].copy_from_slice(&mat_ptr.device_address.to_ne_bytes());
                    push_data[16..20].copy_from_slice(&frame_index.to_ne_bytes());
                    push_data[32..44].copy_from_slice(bytemuck::cast_slice(&eye.to_array()));
                    push_data[48..112].copy_from_slice(bytemuck::cast_slice(&Mat4::IDENTITY.to_cols_array()));
                    push_data[112..176].copy_from_slice(bytemuck::cast_slice(&view_proj.to_cols_array()));

                    forge.device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.layout, 0, &[accum_set], &[]);
                    forge.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.graphics_pipeline);
                    forge.device.cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, &push_data);
                    
                    forge.device.cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: swapchain.extent.height as f32, width: swapchain.extent.width as f32, height: -(swapchain.extent.height as f32), min_depth: 0.0, max_depth: 1.0 }]);
                    forge.device.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: swapchain.extent }]);
                    forge.device.cmd_draw(cmd, header.vertex_count as u32, 1, 0, 0);
                    
                    forge.device.cmd_end_rendering(cmd);
                    let _ = forge.device.end_command_buffer(cmd).unwrap();
                }
                renderer.end_frame(&forge, &swapchain, img_idx);
                frame_index += 1;
            }

            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                unsafe {
                    let _ = forge.device.device_wait_idle();
                    forge.device.destroy_descriptor_pool(descriptor_pool, None);
                    if let Some(u) = universe.take() { u.destroy(&forge.memory); }
                    if let Some(s) = staging.take() { s.destroy(&forge.memory); }
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => (),
        }
    });
}