use ash::vk;
use crate::context::ForgeContext;
use crate::swapchain::ForgeSwapchain;

pub struct ForgeRenderer {
    pub command_pool: vk::CommandPool,
    pub command_buffer: vk::CommandBuffer,
    // Objets de synchronisation
    pub image_available_sem: vk::Semaphore,
    pub render_finished_sem: vk::Semaphore,
    pub in_flight_fence: vk::Fence,
}

impl ForgeRenderer {
    pub fn new(context: &ForgeContext) -> Self {
        unsafe {
            // Pool de commandes (Transient pour reset à chaque frame)
            let pool_info = vk::CommandPoolCreateInfo::builder()
                .queue_family_index(context.queue_family)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

            let command_pool = context.device.create_command_pool(&pool_info, None).unwrap();

            let alloc_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            let command_buffer = context.device.allocate_command_buffers(&alloc_info).unwrap()[0];

            // Création des feux de signalisation (Semaphores & Fences)
            let sem_info = vk::SemaphoreCreateInfo::default();
            let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

            let image_available_sem = context.device.create_semaphore(&sem_info, None).unwrap();
            let render_finished_sem = context.device.create_semaphore(&sem_info, None).unwrap();
            let in_flight_fence = context.device.create_fence(&fence_info, None).unwrap();

            Self {
                command_pool,
                command_buffer,
                image_available_sem,
                render_finished_sem,
                in_flight_fence,
            }
        }
    }

    /// Prépare le GPU pour une nouvelle frame
    pub fn begin_frame(&self, context: &ForgeContext) {
        unsafe {
            // On attend que la frame précédente soit terminée sur le GPU
            context.device.wait_for_fences(&[self.in_flight_fence], true, u64::MAX).unwrap();
            context.device.reset_fences(&[self.in_flight_fence]).unwrap();
        }
    }

    /// Soumet les commandes et présente l'image à l'écran
    pub fn end_frame(&self, context: &ForgeContext, swapchain: &ForgeSwapchain, image_index: u32) {
        unsafe {
            let wait_semaphores = [self.image_available_sem];
            let signal_semaphores = [self.render_finished_sem];
            let command_buffers = [self.command_buffer];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

            // 1. SOUMISSION (Queue Submit)
            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);

            context.device.queue_submit(
                context.queue, 
                &[submit_info.build()], 
                self.in_flight_fence
            ).expect("❌ Échec soumission GPU");

            // 2. PRÉSENTATION (Queue Present)
            let swapchains = [swapchain.handle];
            let image_indices = [image_index];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            swapchain.loader.queue_present(context.queue, &present_info)
                .expect("❌ Échec présentation écran");
        }
    }

    pub fn destroy(&self, context: &ForgeContext) {
        unsafe {
            context.device.destroy_semaphore(self.image_available_sem, None);
            context.device.destroy_semaphore(self.render_finished_sem, None);
            context.device.destroy_fence(self.in_flight_fence, None);
            context.device.destroy_command_pool(self.command_pool, None);
        }
    }
}