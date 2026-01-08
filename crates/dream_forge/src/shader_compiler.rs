use ash::vk;
use shaderc::{Compiler, CompileOptions, ShaderKind};
use std::fs;
use std::path::Path;
use std::fmt;

/// Erreur de compilation shader
#[derive(Debug)]
pub enum ShaderError {
    IoError(std::io::Error),
    CompileError(String),
    VulkanError(vk::Result),
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShaderError::IoError(e) => write!(f, "IO Error: {}", e),
            ShaderError::CompileError(msg) => write!(f, "Compile Error: {}", msg),
            ShaderError::VulkanError(e) => write!(f, "Vulkan Error: {:?}", e),
        }
    }
}

impl std::error::Error for ShaderError {}

impl From<std::io::Error> for ShaderError {
    fn from(err: std::io::Error) -> Self {
        ShaderError::IoError(err)
    }
}

pub struct ShaderCompiler {
    compiler: Compiler,
}

impl ShaderCompiler {
    pub fn new() -> Self {
        Self {
            compiler: Compiler::new().expect("❌ Impossible d'initialiser Shaderc"),
        }
    }

    /// Compile un fichier shader GLSL en module Vulkan
    /// 
    /// # Returns
    /// - `Ok(ShaderModule)` si la compilation réussit
    /// - `Err(ShaderError)` si lecture, compilation ou création échoue
    pub fn compile_file(
        &self,
        device: &ash::Device,
        path: &Path,
        kind: ShaderKind,
    ) -> Result<vk::ShaderModule, ShaderError> {
        let source = fs::read_to_string(path)?;
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.glsl");

        // 1. Options de compilation (Optimisation max pour 2030)
        let mut options = CompileOptions::new().unwrap();
        options.set_target_env(shaderc::TargetEnv::Vulkan, shaderc::EnvVersion::Vulkan1_3 as u32);
        options.set_optimization_level(shaderc::OptimizationLevel::Performance);

        // 🚀 Injection des macros de préprocesseur
        match kind {
            ShaderKind::Vertex => { options.add_macro_definition("VERTEX_SHADER", Some("1")); },
            ShaderKind::Fragment => { options.add_macro_definition("FRAGMENT_SHADER", Some("1")); },
            ShaderKind::Compute => { options.add_macro_definition("COMPUTE_SHADER", Some("1")); },
            _ => {},
        }

        // 2. Compilation en SPIR-V
        let artifact = self.compiler
            .compile_into_spirv(&source, kind, file_name, "main", Some(&options))
            .map_err(|e| {
                log::error!("🔴 [SHADER ERROR] Impossible de compiler {}: {}", file_name, e);
                ShaderError::CompileError(e.to_string())
            })?;

        // 3. Création du module Vulkan
        let create_info = vk::ShaderModuleCreateInfo::builder()
            .code(artifact.as_binary());

        unsafe {
            device.create_shader_module(&create_info, None)
                .map_err(ShaderError::VulkanError)
        }
    }
}

impl Default for ShaderCompiler {
    fn default() -> Self {
        Self::new()
    }
}