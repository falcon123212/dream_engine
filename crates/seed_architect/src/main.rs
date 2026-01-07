use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use dream_forge::{
    context::ForgeContext,
    memory::{StagingBelt, MegaBuffer},
};
use seed_architect::importer::SeedImporter;
use log::info;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("👁️ [LUCID] Démarrage du système de rendu spectral...");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("DREAM ENGINE | SPECTRAL ARCHITECT 2030")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
        .build(&event_loop)
        .expect("❌ Fenêtre KO");

    let forge = ForgeContext::init(&window, "Dream Engine");

    // --- SAO MEMORY SETUP ---
    let mut staging_belt = StagingBelt::new(&forge.memory, 256 * 1024 * 1024); // 256MB pour textures
    let mut mega_buffer = MegaBuffer::new(&forge.memory, 1024 * 1024 * 1024); // 1GB VRAM

    // --- ASSET INGESTION ---
    // 1. On importe les Atomes, la Lumière et les Matériaux
    let path = "assets/raw/relic_test.obj";
    let (geometry, materials) = SeedImporter::import_full_scene(path);

    // 2. Upload des Matériaux vers le MegaBuffer
    // On obtient un GpuPtr qui sera utilisé par le shader pour le Shade-Before-Hit
    let (mat_offset, mat_ptr) = mega_buffer.allocate::<seed_architect::importer::MaterialData>(
        (materials.len() * std::mem::size_of::<seed_architect::importer::MaterialData>()) as u64, 
        16
    );
    staging_belt.push(&materials);

    // 3. Upload de la Géométrie
    let (geo_offset, geo_ptr) = mega_buffer.allocate::<f32>((geometry.len() * 4) as u64, 16);
    staging_belt.push(&geometry);

    info!("🌈 [DATA] Scène chargée : {} matériaux, {} atomes.", materials.len(), geometry.len() / 3);
    info!("📍 [PTR] Adresse Matériaux : 0x{:x}", mat_ptr.device_address);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                info!("🛑 Arrêt.");
                *control_flow = ControlFlow::Exit;
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            },
            Event::RedrawRequested(_) => {
                // Ici le Renderer utilisera mat_ptr et geo_ptr via Push Constants
                staging_belt.reset();
            },
            _ => (),
        }
    });
}