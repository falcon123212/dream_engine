use serde::{Serialize, Deserialize};

pub mod importer;
pub mod abc_loader;

#[repr(C)]
#[derive(Serialize, Deserialize, Debug)]
pub struct SeedFileHeader {
    pub magic: [u8; 4],      // "SEED"
    pub version: u32,        // 2030
    pub vertex_count: u64,   // Nombre d'atomes
    pub index_count: u64,    // Reservé
    pub bvh_offset: u64,     // Reservé pour l'accélération
    pub material_ptr: u64,   // Reservé
}

/// Encodeur Morton 3D (Z-Order Curve)
/// Entrelace les bits de X, Y, Z pour garantir que des points proches 
/// dans l'espace 3D soient proches en mémoire.
pub fn encode_morton_3d(x: u32, y: u32, z: u32) -> u64 {
    let x = expand_bits(x);
    let y = expand_bits(y);
    let z = expand_bits(z);
    x | (y << 1) | (z << 2)
}

// Ecarte les bits (ex: 1111 -> 1001001001)
fn expand_bits(v: u32) -> u64 {
    let mut v = v as u64 & 0x1FFFFF; 
    v = (v | (v << 32)) & 0x1F00000000FFFF;
    v = (v | (v << 16)) & 0x1F0000FF0000FF;
    v = (v | (v << 8))  & 0x100F00F00F00F00F;
    v = (v | (v << 4))  & 0x10C30C30C30C30C3;
    v = (v | (v << 2))  & 0x1249249249249249;
    v
}