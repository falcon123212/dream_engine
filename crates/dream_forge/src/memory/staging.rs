use ash::vk;
use gpu_allocator::vulkan::Allocation;
use gpu_allocator::MemoryLocation;
use std::ptr::copy_nonoverlapping;
use super::manager::MemoryManager;

pub struct StagingBelt {
    buffer: vk::Buffer,
    allocation: Allocation, // On garde l'allocation pour ne pas la drop
    ptr: *mut u8,           // Pointeur brut mappé (CPU Write)
    capacity: u64,
    head: u64,              // Position actuelle d'écriture
}

// StagingBelt n'est pas thread-safe par défaut, on le gère plus haut.
unsafe impl Send for StagingBelt {}

impl StagingBelt {
    pub fn new(mem_manager: &MemoryManager, capacity: u64) -> Self {
        // Usage: TRANSFER_SRC (Source de copie)
        let (buffer, allocation) = mem_manager.create_buffer(
            capacity,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryLocation::CpuToGpu, // Mappé CPU, visible GPU
            "Staging Belt Ring",
        );

        let ptr = allocation.mapped_ptr().expect("Staging Buffer doit être mappable !")
            .as_ptr() as *mut u8;

        println!("🚚 [MEMORY] Staging Belt de {} MB alloué.", capacity / 1024 / 1024);

        Self {
            buffer,
            allocation,
            ptr,
            capacity,
            head: 0,
        }
    }

    /// Pousse des données dans le Ring. 
    /// Retourne (Buffer, Offset) pour la commande de copie.
    pub fn push<T: Copy>(&mut self, data: &[T]) -> (vk::Buffer, u64) {
        let size = (std::mem::size_of::<T>() * data.len()) as u64;
        let align = 256; // Alignement standard safe pour Vulkan offsets
        
        // Calcul du padding pour l'alignement
        let padding = (align - (self.head % align)) % align;
        let start_offset = self.head + padding;
        
        // Sécurité Ring Buffer (Version Simple : Panic si plein)
        // Dans le Step 2, on ajoutera l'attente intelligente (Fences)
        if start_offset + size > self.capacity {
            panic!("❌ STAGING BELT FULL ! Augmentez la capacité ou attendez la frame suivante.");
        }

        unsafe {
            let dest = self.ptr.add(start_offset as usize);
            // Copie Memcpy ultra-rapide
            copy_nonoverlapping(data.as_ptr() as *const u8, dest, size as usize);
        }

        self.head = start_offset + size;

        (self.buffer, start_offset)
    }

    /// Appelé à la fin de la frame pour remettre le pointeur à zéro
    /// (NOTE: Ne faire ça que si on est sûr que le GPU a fini de lire !)
    pub fn reset(&mut self) {
        self.head = 0;
    }

    pub fn destroy(self, mem_manager: &MemoryManager) {
        mem_manager.destroy_buffer(self.buffer, self.allocation);
    }
}