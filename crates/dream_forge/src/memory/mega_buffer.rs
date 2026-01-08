use ash::vk;
use gpu_allocator::vulkan::Allocation;
use gpu_allocator::MemoryLocation;
use dream_core::types::GpuPtr;
use crate::memory::manager::MemoryManager;

pub struct MegaBuffer {
    buffer: vk::Buffer,
    allocation: Allocation,
    pub device_address: u64, // Adresse BDA pour les Shaders
    capacity: u64,
    cursor: u64,
}

impl MegaBuffer {
    pub fn new(mem_manager: &MemoryManager, capacity: u64) -> Self {
        // 1. FLAGS ÉTENDUS : On ajoute TRANSFER_SRC pour permettre au CPU de relire les calculs du GPU
        let usage = vk::BufferUsageFlags::TRANSFER_DST 
            | vk::BufferUsageFlags::TRANSFER_SRC // 🟢 Requis pour lire le résultat du Ray-Cast
            | vk::BufferUsageFlags::STORAGE_BUFFER 
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;

        let (buffer, allocation) = mem_manager.create_buffer(
            capacity,
            usage,
            // 2. LOCATION UNIVERSELLE : CpuToGpu (Requis pour l'interaction bidirectionnelle)
            MemoryLocation::CpuToGpu, 
            "Mega Buffer Universe",
        );

        // Récupération de l'adresse GPU réelle (BDA)
        let addr_info = vk::BufferDeviceAddressInfo::builder().buffer(buffer);
        let device_address = unsafe { 
            mem_manager.get_device().get_buffer_device_address(&addr_info) 
        };

        println!("🌌 [MEMORY] MegaBuffer Interactif alloué : {} MB", capacity / 1024 / 1024);

        Self {
            buffer,
            allocation,
            device_address,
            capacity,
            cursor: 0,
        }
    }

    /// 🆕 MÉTHODE D'INTERACTION : Lit une valeur (ex: l'index de l'atome cliqué)
    /// Utilisé par le Protocole Fantôme pour identifier ce que l'utilisateur touche.
    pub fn read_value<T: Copy>(&self, offset: u64) -> T {
        unsafe {
            let mapped_ptr = self.allocation.mapped_ptr()
                .expect("❌ MegaBuffer doit être mappé pour l'interaction CPU")
                .as_ptr() as *const u8;
            
            let target_ptr = mapped_ptr.add(offset as usize) as *const T;
            *target_ptr
        }
    }

    pub fn buffer_handle(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn allocate<T>(&mut self, size: u64, align: u64) -> (u64, GpuPtr<T>) {
        let remainder = self.cursor % align;
        let padding = if remainder == 0 { 0 } else { align - remainder };
        
        let start = self.cursor + padding;
        
        if start + size > self.capacity {
            panic!("❌ [CRITICAL] VRAM SATURÉE : Allocation de {} octets impossible", size);
        }

        self.cursor = start + size;
        let ptr_address = self.device_address + start;
        
        (start, GpuPtr::new(ptr_address))
    }

    pub fn destroy(self, mem_manager: &MemoryManager) {
        mem_manager.destroy_buffer(self.buffer, self.allocation);
        println!("🧹 [MEMORY] MegaBuffer libéré.");
    }
}