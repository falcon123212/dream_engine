use ash::vk;
use shaderc::{Compiler, CompileOptions, ShaderKind};
use std::fs;
use std::path::Path;

pub struct ShaderCompiler {
    compiler: Compiler,
}

impl ShaderCompiler {
    pub fn new() -> Self {
        Self {
            compiler: Compiler::new().expect("❌ Impossible d'initialiser Shaderc"),
        }
    }

    pub fn compile_file(
        &self,
        device: &ash::Device,
        path: &Path,
        kind: ShaderKind,
    ) -> vk::ShaderModule {
        let source = fs::read_to_string(path).expect("❌ Impossible de lire le shader");
        let file_name = path.file_name().unwrap().to_str().unwrap();

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

        // 2. Compilation en SPIR-V avec gestion d'erreurs
        let artifact = match self.compiler.compile_into_spirv(&source, kind, file_name, "main", Some(&options)) {
            Ok(binary) => binary,
            Err(e) => {
                log::error!("🔴 [SHADER ERROR] Impossible de compiler {}: {}", file_name, e);
                return vk::ShaderModule::null(); 
            }
        };

        // 3. Création du module Vulkan
        let create_info = vk::ShaderModuleCreateInfo::builder()
            .code(artifact.as_binary());

        unsafe {
            device.create_shader_module(&create_info, None)
                .expect("❌ Échec vkCreateShaderModule")
        }
    }
}