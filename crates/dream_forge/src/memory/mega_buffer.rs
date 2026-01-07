use ash::vk;
use gpu_allocator::vulkan::Allocation;
use gpu_allocator::MemoryLocation;
use dream_core::types::GpuPtr;
use crate::memory::manager::MemoryManager;

pub struct MegaBuffer {
    buffer: vk::Buffer,
    allocation: Allocation,
    device_address: u64,
    capacity: u64,
    cursor: u64,
}

impl MegaBuffer {
    pub fn new(mem_manager: &MemoryManager, capacity: u64) -> Self {
        let usage = vk::BufferUsageFlags::TRANSFER_DST 
            | vk::BufferUsageFlags::STORAGE_BUFFER 
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;

        let (buffer, allocation) = mem_manager.create_buffer(
            capacity,
            usage,
            MemoryLocation::GpuOnly,
            "Mega Buffer Universe",
        );

        let addr_info = vk::BufferDeviceAddressInfo::builder().buffer(buffer);
        let device_address = unsafe { 
            mem_manager.get_device().get_buffer_device_address(&addr_info) 
        };

        println!("🌌 [MEMORY] MegaBuffer alloué : {} MB", capacity / 1024 / 1024);

        Self {
            buffer,
            allocation,
            device_address,
            capacity,
            cursor: 0,
        }
    }

    // 🚀 La version générique pour corriger les erreurs de types
    pub fn allocate<T>(&mut self, size: u64, align: u64) -> (u64, GpuPtr<T>) {
        let padding = (align - (self.cursor % align)) % align;
        let start = self.cursor + padding;
        
        if start + size > self.capacity {
            panic!("❌ VRAM SATURÉE (MegaBuffer) !");
        }

        self.cursor = start + size;
        let ptr_address = self.device_address + start;
        
        // On retourne un pointeur typé
        (start, GpuPtr::new(ptr_address))
    }

    pub fn destroy(self, mem_manager: &MemoryManager) {
        mem_manager.destroy_buffer(self.buffer, self.allocation);
    }
}