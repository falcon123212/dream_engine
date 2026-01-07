use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use dream_forge::{
    ForgeContext, ForgeRenderer, ForgeSwapchain, PipelineManager,
    ShaderCompiler, shader_watcher::create_shader_watcher,
    memory::{StagingBelt, MegaBuffer},
};
use seed_architect::importer::SeedImporter;

use log::info;
use ash::vk;
use glam::{Mat4, Vec3};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("👁️ [LUCID] Démarrage du moteur SAO 2030...");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("DREAM ENGINE | SPECTRAL ARCHITECT")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
        .build(&event_loop).unwrap();

    // --- 1. INITIALISATION FORGE ---
    let forge = ForgeContext::init(&window, "Dream Engine");
    let renderer = ForgeRenderer::new(&forge);
    let mut swapchain = ForgeSwapchain::new(&forge, &window);
    let shader_compiler = ShaderCompiler::new();
    let (_watcher, shader_events) = create_shader_watcher();

    // --- 2. MÉMOIRE ---
    let mut staging = Some(StagingBelt::new(&forge.memory, 256 * 1024 * 1024));
    let mut universe = Some(MegaBuffer::new(&forge.memory, 1024 * 1024 * 1024));

    // --- 3. ASSETS (L'import renvoie des types précis) ---
    // --- 4. INGESTION ASSETS (Typage explicite) ---
    // On précise à Rust que ce sont des vecteurs de f32 et de matériaux
    let (geometry, materials): (Vec<f32>, Vec<seed_architect::importer::MaterialData>) = 
        SeedImporter::import_full_scene("assets/raw/relic_test.obj");
    
    // Allocation avec Turbo-fish ::<f32> pour lever l'ambiguïté
    let (_geo_off, geo_ptr) = universe.as_mut().unwrap().allocate::<f32>(
        (geometry.len() * std::mem::size_of::<f32>()) as u64, 
        16
    );
    staging.as_mut().unwrap().push(&geometry);

    // Allocation pour les matériaux
    let (_mat_off, mat_ptr) = universe.as_mut().unwrap().allocate::<seed_architect::importer::MaterialData>(
         (materials.len() * std::mem::size_of::<seed_architect::importer::MaterialData>()) as u64, 
        16
    );
    staging.as_mut().unwrap().push(&materials);
    


    // --- 4. PIPELINE & SHADERS ---
    // --- 4. PIPELINE & SHADERS ---
    let shader_path = std::path::Path::new("assets/shaders/surface.glsl");
    let vert_mod = shader_compiler.compile_file(&forge.device, shader_path, shaderc::ShaderKind::Vertex);
    let frag_mod = shader_compiler.compile_file(&forge.device, shader_path, shaderc::ShaderKind::Fragment);

    if vert_mod == vk::ShaderModule::null() || frag_mod == vk::ShaderModule::null() {
        panic!("❌ [CRITICAL] Échec de la compilation des shaders. Arrêt d'urgence.");
    }

    let mut pipeline = PipelineManager::new(&forge, vert_mod, frag_mod, swapchain.format);

    info!("✅ [SYSTEM] Atomes chargés en VRAM.");

    // --- 5. BOUCLE ---
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => {
                if let Ok(Ok(_)) = shader_events.try_recv() {
                    info!("🔄 [RELOAD] Recompilation...");
                    let v = shader_compiler.compile_file(&forge.device, shader_path, shaderc::ShaderKind::Vertex);
                    let f = shader_compiler.compile_file(&forge.device, shader_path, shaderc::ShaderKind::Fragment);
                    pipeline = PipelineManager::new(&forge, v, f, swapchain.format);
                }
                window.request_redraw();
            }

            Event::RedrawRequested(_) => {
                renderer.begin_frame(&forge);
                let (img_idx, _) = unsafe { swapchain.loader.acquire_next_image(swapchain.handle, u64::MAX, renderer.image_available_sem, vk::Fence::null()).unwrap() };

                unsafe {
                    let cmd = renderer.command_buffer;
                    forge.device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty()).unwrap();
                    forge.device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).unwrap();

                    let color_attach = vk::RenderingAttachmentInfo::builder()
                        .image_view(swapchain.image_views[img_idx as usize])
                        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.01, 0.01, 0.015, 1.0] } });

                    // Correction 3: Transition explicite Undefined -> ColorAttachment
                    let image_barrier = vk::ImageMemoryBarrier::builder()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .image(swapchain.images[img_idx as usize])
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .build();

                    forge.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[image_barrier],
                    );

                    let render_info = vk::RenderingInfo::builder()
                        .render_area(vk::Rect2D { extent: swapchain.extent, ..Default::default() })
                        .layer_count(1)
                        .color_attachments(std::slice::from_ref(&color_attach));

                    forge.device.cmd_begin_rendering(cmd, &render_info);
                    forge.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.graphics_pipeline);

                    let view_proj = Mat4::perspective_rh(45.0f32.to_radians(), 1.77, 0.1, 1000.0);
                    let model = Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0));
                    
                    let mut push_data = [0u8; 144];
                    push_data[0..8].copy_from_slice(&geo_ptr.device_address.to_ne_bytes());
                    push_data[8..16].copy_from_slice(&mat_ptr.device_address.to_ne_bytes());
                    push_data[16..80].copy_from_slice(bytemuck::cast_slice(&model.to_cols_array()));
                    push_data[80..144].copy_from_slice(bytemuck::cast_slice(&view_proj.to_cols_array()));

                    forge.device.cmd_push_constants(cmd, pipeline.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, &push_data);
                    
                    forge.device.cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: swapchain.extent.width as f32, height: swapchain.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
                    forge.device.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: swapchain.extent }]);
                    
                    forge.device.cmd_draw(cmd, (geometry.len() / 3) as u32, 1, 0, 0);

                    forge.device.cmd_end_rendering(cmd);

                    // Correction 4: Transition vers PRESENT_SRC_KHR pour l'affichage
                    let present_barrier = vk::ImageMemoryBarrier::builder()
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .image(swapchain.images[img_idx as usize])
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .dst_access_mask(vk::AccessFlags::empty()) // Le sémaphore de présentation gère la synchro externe
                        .build();

                    forge.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[present_barrier],
                    );
                    forge.device.end_command_buffer(cmd).unwrap();
                }

                renderer.end_frame(&forge, &swapchain, img_idx);
                if let Some(staging) = staging.as_mut() {
                    staging.reset();
                }
            }

            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            
            Event::LoopDestroyed => {
                info!("👋 [LUCID] Nettoyage des ressources...");
                unsafe {
                    let _ = forge.device.device_wait_idle();
                }
                renderer.destroy(&forge);
                swapchain.destroy(&forge);
                if let Some(staging) = staging.take() {
                    staging.destroy(&forge.memory);
                }
                if let Some(universe) = universe.take() {
                    universe.destroy(&forge.memory);
                }
            },
            
            _ => (),
        }
    });
}