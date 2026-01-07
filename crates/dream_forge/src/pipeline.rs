use ash::vk;
use crate::context::ForgeContext;
use std::ffi::CString;

pub struct PipelineManager {
    pub layout: vk::PipelineLayout,
    pub graphics_pipeline: vk::Pipeline,
}

impl PipelineManager {
    pub fn new(
        context: &ForgeContext,
        vert_shader: vk::ShaderModule,
        frag_shader: vk::ShaderModule,
        color_format: vk::Format,
    ) -> Self {
        unsafe {
            let entry_name = CString::new("main").unwrap();

            // 1. Shaders Stages
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

            // 2. Push Constants (Le lien vers nos pointeurs BDA)
            // Doit matcher la struct pc dans surface.glsl
            let push_constant_range = vk::PushConstantRange::builder()
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                .offset(0)
                .size(144); // 2 ptr (16) + 2 mat4 (128) = 144 bytes

            let layout_info = vk::PipelineLayoutCreateInfo::builder()
                .push_constant_ranges(std::slice::from_ref(&push_constant_range));
            
            let layout = context.device.create_pipeline_layout(&layout_info, None).unwrap();

            // 3. Dynamic Rendering & Pipeline States
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default(); // Bindless = Pas d'input
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

            let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
                .line_width(1.0)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .polygon_mode(vk::PolygonMode::FILL);

            let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);

            let color_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .blend_enable(false);

            let color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
                .attachments(std::slice::from_ref(&color_blend_attachment));

            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_info = vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(&dynamic_states);

            // 4. Intégration Vulkan 1.3 (Rendering Info)
            let color_formats = [color_format];
            let mut rendering_info = vk::PipelineRenderingCreateInfo::builder()
                .color_attachment_formats(&color_formats);
            
            // Correction 2: Viewport State ne doit pas être null même en dynamique
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
                .color_blend_state(&color_blending)
                .dynamic_state(&dynamic_info)
                .viewport_state(&viewport_state) // <-- AJOUT CRITIQUE ICI
                .layout(layout);

            let graphics_pipeline = context.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&pipeline_info),
                None,
            ).expect("❌ Échec création Pipeline")[0];

            Self { layout, graphics_pipeline }
        }
    }
}