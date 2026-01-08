use ash::{vk, extensions::khr};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use crate::context::ForgeContext;
use log::{info, error};
use gpu_allocator::vulkan::Allocation;
use gpu_allocator::MemoryLocation;

pub struct ForgeSwapchain {
    pub surface_loader: khr::Surface,
    pub surface: vk::SurfaceKHR,
    pub loader: khr::Swapchain,
    pub handle: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub needs_resize: bool,
    
    // --- Ressources Depth ---
    pub depth_image: vk::Image,
    pub depth_view: vk::ImageView,
    pub depth_allocation: Option<Allocation>,
    pub depth_format: vk::Format,

    // 🆕 Ressources d'Accumulation Temporelle
    pub accum_image: vk::Image,
    pub accum_view: vk::ImageView,
    pub accum_allocation: Option<Allocation>,
    pub accum_format: vk::Format,
}

impl ForgeSwapchain {
    pub fn new(context: &ForgeContext, window: &(impl HasRawWindowHandle + HasRawDisplayHandle)) -> Self {
        unsafe {
            let surface_loader = khr::Surface::new(&context.entry, &context.instance);
            let surface = ash_window::create_surface(
                &context.entry,
                &context.instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            ).expect("❌ Échec création Surface");

            let loader = khr::Swapchain::new(&context.instance, &context.device);
            
            let capabilities = surface_loader
                .get_physical_device_surface_capabilities(context.physical_device, surface)
                .expect("❌ Échec récupération capabilities surface");
            
            let extent = if capabilities.current_extent.width != u32::MAX {
                capabilities.current_extent
            } else {
                vk::Extent2D { width: 1280, height: 720 }
            };
            
            let format = vk::Format::B8G8R8A8_UNORM;
            
            let image_count = 3.max(capabilities.min_image_count)
                .min(if capabilities.max_image_count > 0 { capabilities.max_image_count } else { 3 });

            let create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(image_count)
                .image_format(format)
                .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::FIFO);

            let handle = loader.create_swapchain(&create_info, None)
                .expect("❌ Échec création Swapchain");
            
            let images = loader.get_swapchain_images(handle).unwrap();
            let image_views = Self::create_image_views(context, &images, format);

            // Ressources Depth
            let depth_format = vk::Format::D32_SFLOAT;
            let (depth_image, depth_view, depth_allocation) = Self::create_image_resource(context, extent, depth_format, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT, "Depth Buffer");

            // 🆕 Ressources Accumulation (HDR 32-bit pour le Path Tracing)
            let accum_format = vk::Format::R32G32B32A32_SFLOAT;
            let (accum_image, accum_view, accum_allocation) = Self::create_image_resource(
                context, 
                extent, 
                accum_format, 
                vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST,
                "Accumulation Buffer"
            );

            info!("🖼️ [SWAPCHAIN] Créée: {}x{} ({} images) + Depth + Accumulation HDR", extent.width, extent.height, images.len());

            Self { 
                surface_loader, surface, loader, handle, images, image_views, 
                format, extent, needs_resize: false,
                depth_image, depth_view, depth_allocation: Some(depth_allocation), depth_format,
                accum_image, accum_view, accum_allocation: Some(accum_allocation), accum_format,
            }
        }
    }

    /// 🆕 Helper universel pour créer des ressources images (Depth/Accum)
    unsafe fn create_image_resource(
        context: &ForgeContext, 
        extent: vk::Extent2D, 
        format: vk::Format, 
        usage: vk::ImageUsageFlags,
        name: &str
    ) -> (vk::Image, vk::ImageView, Allocation) {
        let create_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let (image, allocation) = context.memory.create_image(&create_info, MemoryLocation::GpuOnly, name);

        let aspect_mask = if name.contains("Depth") { vk::ImageAspectFlags::DEPTH } else { vk::ImageAspectFlags::COLOR };

        let view_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        
        let view = context.device.create_image_view(&view_info, None).expect("❌ create_view failed");

        (image, view, allocation)
    }

    pub fn recreate(&mut self, context: &ForgeContext, new_extent: vk::Extent2D) {
        unsafe {
            if let Err(e) = context.device.device_wait_idle() {
                error!("⚠️ [SWAPCHAIN] Impossible d'attendre le GPU avant resize: {:?}", e);
                self.needs_resize = true;
                return; 
            }
            
            let capabilities = self.surface_loader
                .get_physical_device_surface_capabilities(context.physical_device, self.surface)
                .expect("❌ [SWAPCHAIN] Échec capabilities");
            
            let clamped_extent = vk::Extent2D {
                width: new_extent.width.clamp(capabilities.min_image_extent.width.max(1), capabilities.max_image_extent.width),
                height: new_extent.height.clamp(capabilities.min_image_extent.height.max(1), capabilities.max_image_extent.height),
            };
            
            if clamped_extent.width == 0 || clamped_extent.height == 0 { return; }
            
            // Nettoyage complet
            for &view in &self.image_views { context.device.destroy_image_view(view, None); }
            context.device.destroy_image_view(self.depth_view, None);
            if let Some(alloc) = self.depth_allocation.take() { context.memory.destroy_image(self.depth_image, alloc); }
            
            context.device.destroy_image_view(self.accum_view, None);
            if let Some(alloc) = self.accum_allocation.take() { context.memory.destroy_image(self.accum_image, alloc); }

            let old_swapchain = self.handle;
            let create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(self.surface)
                .old_swapchain(old_swapchain)
                .min_image_count(3.max(capabilities.min_image_count))
                .image_format(self.format)
                .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                .image_extent(clamped_extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
                .pre_transform(capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::FIFO);

            self.handle = self.loader.create_swapchain(&create_info, None).expect("❌ Swapchain Recreate KO");
            self.loader.destroy_swapchain(old_swapchain, None);
            
            self.images = self.loader.get_swapchain_images(self.handle).unwrap();
            self.image_views = Self::create_image_views(context, &self.images, self.format);
            
            // Recréer Depth + Accumulation
            let (d_img, d_view, d_alloc) = Self::create_image_resource(context, clamped_extent, self.depth_format, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT, "Depth Buffer");
            self.depth_image = d_img; self.depth_view = d_view; self.depth_allocation = Some(d_alloc);
            
            let (a_img, a_view, a_alloc) = Self::create_image_resource(context, clamped_extent, self.accum_format, vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST, "Accumulation Buffer");
            self.accum_image = a_img; self.accum_view = a_view; self.accum_allocation = Some(a_alloc);
            
            self.extent = clamped_extent;
            self.needs_resize = false;
        }
    }

    fn create_image_views(context: &ForgeContext, images: &[vk::Image], format: vk::Format) -> Vec<vk::ImageView> {
        images.iter().map(|&img| {
            let view_info = vk::ImageViewCreateInfo::builder()
                .image(img)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    level_count: 1,
                    layer_count: 1,
                    ..Default::default()
                });
            unsafe { context.device.create_image_view(&view_info, None).unwrap() }
        }).collect()
    }

 pub fn acquire_next_image(&mut self, semaphore: vk::Semaphore) -> Option<u32> {
    unsafe {
        match self.loader.acquire_next_image(self.handle, u64::MAX, semaphore, vk::Fence::null()) {
            Ok((idx, false)) => Some(idx),
            // On sépare les deux cas ou on utilise un underscore pour ignorer l'index
            Ok((_idx, true)) => { 
                self.needs_resize = true; 
                None 
            },
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => { 
                self.needs_resize = true; 
                None 
            },
            Err(e) => { 
                error!("❌ acquire failed: {:?}", e); 
                self.needs_resize = true; 
                None 
            },
        }
    }
}
    pub fn destroy(&mut self, context: &ForgeContext) {
        unsafe {
            let _ = context.device.device_wait_idle(); 
            for &view in &self.image_views { context.device.destroy_image_view(view, None); }
            context.device.destroy_image_view(self.depth_view, None);
            if let Some(alloc) = self.depth_allocation.take() { context.memory.destroy_image(self.depth_image, alloc); }
            
            context.device.destroy_image_view(self.accum_view, None);
            if let Some(alloc) = self.accum_allocation.take() { context.memory.destroy_image(self.accum_image, alloc); }
            
            self.loader.destroy_swapchain(self.handle, None);
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}