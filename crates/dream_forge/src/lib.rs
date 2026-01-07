pub mod context;
pub mod memory;    // Contient manager, staging, mega_buffer, abc_streamer
pub mod renderer;
pub mod swapchain;
pub mod pipeline;
pub mod shader_compiler;
pub mod shader_watcher;

// Raccourcis (Ré-exports) pour que main.rs ne change pas
pub use context::ForgeContext;
pub use renderer::ForgeRenderer;
pub use swapchain::ForgeSwapchain;
pub use pipeline::PipelineManager;
pub use shader_compiler::ShaderCompiler;