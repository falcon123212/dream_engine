use ash::vk;
use gpu_allocator::vulkan::*;
use gpu_allocator::MemoryLocation;
use std::sync::{Arc, Mutex};
use log::info;

pub struct MemoryManager {
    allocator: Arc<Mutex<Allocator>>,
    device: ash::Device,
}

impl MemoryManager {
    /// Crée un nouveau gestionnaire de mémoire basé sur gpu-allocator
    pub fn new(
        instance: &ash::Instance,
        device: &ash::Device,
        p_device: vk::PhysicalDevice,
    ) -> Self {
        let allocator_create_desc = AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device: p_device,
            debug_settings: Default::default(),
            buffer_device_address: true, // Requis pour l'architecture Bindless/BDA
            allocation_sizes: Default::default(),
        };

        let allocator = Allocator::new(&allocator_create_desc)
            .expect("❌ Impossible d'initialiser l'allocateur GPU");

        info!("🧠 [MEMORY] Global Allocator (gpu-allocator) initialisé.");

        Self {
            allocator: Arc::new(Mutex::new(allocator)),
            device: device.clone(),
        }
    }

    /// Accesseur pour le device Vulkan (utilisé par MegaBuffer)
    pub fn get_device(&self) -> &ash::Device {
        &self.device
    }

    /// Alloue et crée un buffer Vulkan avec sa mémoire associée
    pub fn create_buffer(
        &self,
        size: u64,
        usage: vk::BufferUsageFlags,
        location: MemoryLocation,
        name: &str,
    ) -> (vk::Buffer, Allocation) {
        // 1. Définir les infos du buffer
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        // 2. Créer le handle Vulkan
        let buffer = unsafe { 
            self.device.create_buffer(&buffer_info, None)
                .expect("❌ Erreur vkCreateBuffer") 
        };

        // 3. Récupérer les exigences matérielles (alignement, type de mémoire)
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        // 4. Allouer la mémoire physique via gpu-allocator
        let allocation = self.allocator
            .lock()
            .expect("❌ Mutex Allocator corrompu")
            .allocate(&AllocationCreateDesc {
                name,
                requirements,
                location,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .expect("❌ Échec de l'allocation VRAM");

        // 5. Lier la mémoire au buffer
        unsafe {
            self.device
                .bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .expect("❌ Échec vkBindBufferMemory");
        }

        (buffer, allocation)
    }

    /// Libère un buffer et sa mémoire
    pub fn destroy_buffer(&self, buffer: vk::Buffer, allocation: Allocation) {
        unsafe {
            self.device.destroy_buffer(buffer, None);
        }
        self.allocator
            .lock()
            .expect("❌ Mutex Allocator corrompu")
            .free(allocation)
            .expect("❌ Échec de la libération mémoire");
    }
}