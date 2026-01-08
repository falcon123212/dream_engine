use ash::vk;
use crate::context::ForgeContext;
use std::ffi::CString;
use log::info;

pub struct PipelineManager {
    pub layout: vk::PipelineLayout,
    pub graphics_pipeline: vk::Pipeline,
    pub compute_layout: vk::PipelineLayout,
    pub compute_pipeline: vk::Pipeline,
    // 🆕 Nécessaire pour l'accumulation temporelle HDR
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    device: ash::Device,
}

impl PipelineManager {
    pub fn new(
        context: &ForgeContext,
        vert_shader: vk::ShaderModule,
        frag_shader: vk::ShaderModule,
        compute_shader: vk::ShaderModule,
        color_format: vk::Format,
        depth_format: vk::Format,
    ) -> Self {
        unsafe {
            let entry_name = CString::new("main").unwrap();

            // --- 1. CONFIGURATION DES DESCRIPTEURS (Accumulation) ---
            // Binding 0: Storage Image pour l'accumulation HDR progressive
            let accum_binding = vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::COMPUTE);

            let descriptor_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(std::slice::from_ref(&accum_binding));
            
            let descriptor_set_layout = context.device
                .create_descriptor_set_layout(&descriptor_info, None)
                .expect("❌ DescriptorSetLayout KO");

            // --- 2. LAYOUT GRAPHIQUE (Alignement 160 octets) ---
            // On passe à 160 octets pour aligner les mat4 sur des multiples de 16
            let push_constant_range = vk::PushConstantRange::builder()
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                .offset(0)
                .size(176); 

            let layouts = [descriptor_set_layout];
            let layout_info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&layouts)
                .push_constant_ranges(std::slice::from_ref(&push_constant_range));
            
            let layout = context.device.create_pipeline_layout(&layout_info, None).unwrap();

            // --- 3. LAYOUT COMPUTE (Picker & Culling) ---
            let compute_push_range = vk::PushConstantRange::builder()
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .offset(0)
                .size(64);

            let compute_layout_info = vk::PipelineLayoutCreateInfo::builder()
                .push_constant_ranges(std::slice::from_ref(&compute_push_range));
            
            let compute_layout = context.device.create_pipeline_layout(&compute_layout_info, None).unwrap();

            // --- 4. PIPELINE GRAPHIQUE (Point Splatting) ---
            let shader_stages = [
                vk::PipelineShaderStageCreateInfo::builder()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(vert_shader)
                    .name(&entry_name)
                    .build(),
                vk::PipelineShaderStageCreateInfo::builder()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(frag_shader)
                    .name(&entry_name)
                    .build(),
            ];

            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
                .topology(vk::PrimitiveTopology::POINT_LIST)
                .primitive_restart_enable(false);

            let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
                .line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE)
                .polygon_mode(vk::PolygonMode::FILL);

            let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);

            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::builder()
                .depth_test_enable(true)
                .depth_write_enable(true)
                .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);

            let color_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .blend_enable(false)
                .build();

            let color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
                .attachments(std::slice::from_ref(&color_blend_attachment));

            let mut rendering_info = vk::PipelineRenderingCreateInfo::builder()
                .color_attachment_formats(std::slice::from_ref(&color_format))
                .depth_attachment_format(depth_format);

            let dynamic_info = vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);

            let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
                .viewport_count(1)
                .scissor_count(1);

            let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
                .push_next(&mut rendering_info)
                .stages(&shader_stages)
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&input_assembly)
                .rasterization_state(&rasterizer)
                .multisample_state(&multisampling)
                .depth_stencil_state(&depth_stencil)
                .color_blend_state(&color_blending)
                .viewport_state(&viewport_state)
                .dynamic_state(&dynamic_info)
                .layout(layout);

            let graphics_pipeline = context.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&pipeline_info.build()),
                None,
            ).expect("❌ Échec Pipeline Graphique")[0];

            // --- 5. PIPELINE COMPUTE ---
            let compute_stage = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(compute_shader)
                .name(&entry_name)
                .build();

            let compute_info = vk::ComputePipelineCreateInfo::builder()
                .stage(compute_stage)
                .layout(compute_layout);

            let compute_pipeline = context.device.create_compute_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&compute_info.build()),
                None,
            ).expect("❌ Échec Pipeline Compute")[0];

            info!("🎨 [PIPELINE] Systèmes Graphiques et Compute synchronisés (HDR Accumulation ready).");

            Self {
                layout,
                graphics_pipeline,
                compute_layout,
                compute_pipeline,
                descriptor_set_layout,
                device: context.device.clone(),
            }
        }
    }
}

impl Drop for PipelineManager {
    fn drop(&mut self) {
        unsafe {
            info!("🧹 [PIPELINE] Destruction des ressources GPU...");
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device.destroy_pipeline(self.compute_pipeline, None);
            self.device.destroy_pipeline_layout(self.compute_layout, None);
        }
    }
}