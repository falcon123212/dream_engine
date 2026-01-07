// crates/dream_forge/src/memory/abc_streamer.rs

use crate::memory::{StagingBelt, MegaBuffer};
use dream_core::types::GpuPtr;

pub struct AbcStreamer {
    pub vram_ptr: GpuPtr<f32>,
    pub buffer_offset: u64,
}

impl AbcStreamer {
    pub fn update_gpu(
        &mut self, 
        staging: &mut StagingBelt, 
        new_data: &[f32]
    ) {
        // On pousse les données de la frame actuelle sur le tapis roulant
        staging.push(new_data);
        
        // Note : Au prochain Redraw, le CommandBuffer copiera 
        // du Staging vers le vram_ptr dans le MegaBuffer.
    }
}