use ash::{vk, extensions::khr};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use crate::context::ForgeContext;

pub struct ForgeSwapchain {
    pub surface_loader: khr::Surface,
    pub surface: vk::SurfaceKHR,
    pub loader: khr::Swapchain,
    pub handle: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
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
            
            // Configuration basique (V-Sync activé par défaut pour la stabilité)
            let extent = vk::Extent2D { width: 1280, height: 720 };
            let format = vk::Format::B8G8R8A8_UNORM;

            let create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(3) // Triple Buffering SAO
                .image_format(format)
                .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::FIFO);

            let handle = loader.create_swapchain(&create_info, None).expect("❌ Échec Swapchain");
            let images = loader.get_swapchain_images(handle).unwrap();
            
            let image_views: Vec<vk::ImageView> = images.iter().map(|&img| {
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
                context.device.create_image_view(&view_info, None).unwrap()
            }).collect();

            Self { surface_loader, surface, loader, handle, images, image_views, format, extent }
        }
    }

    pub fn destroy(&mut self, context: &ForgeContext) {
        unsafe {
            for &view in &self.image_views {
                context.device.destroy_image_view(view, None);
            }
            self.loader.destroy_swapchain(self.handle, None);
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}