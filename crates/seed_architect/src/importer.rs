use dream_core::types::{GpuPtr, InstanceData};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MaterialData {
    pub base_color: Vec3,
    pub roughness: f32,
    pub metallic: f32,
    pub ior: f32,
    pub emissive_ptr: u64, // GpuPtr vers la texture en VRAM
}

pub struct SeedImporter;

impl SeedImporter {
    pub fn import_full_scene(path: &str) -> (Vec<f32>, Vec<MaterialData>) {
        let load_options = tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ..Default::default()
        };

        let (models, materials_result) = tobj::load_obj(path, &load_options)
            .expect("❌ Erreur Ingestion OBJ");

        // 1. Extraction Matériaux
        let materials = match materials_result {
            Ok(mats) => mats.into_iter().map(|m| MaterialData {
                base_color: Vec3::from_array(m.diffuse.unwrap_or([1.0, 1.0, 1.0])), // Fallback blanc
                roughness: 0.5,
                metallic: 0.0,
                ior: m.optical_density.unwrap_or(1.45),
                emissive_ptr: 0,
            }).collect(),
            Err(_) => vec![MaterialData {
                base_color: Vec3::ONE,
                roughness: 0.5,
                metallic: 0.0,
                ior: 1.45,
                emissive_ptr: 0,
            }], // Fallback si pas de MTL
        };

        // 2. Extraction Géométrie (Flattening indices for non-indexed draw)
        let mut geometry = Vec::new();
        for model in models {
            let mesh = model.mesh;
            for index in mesh.indices {
                let i = index as usize;
                // tobj positions are [x, y, z, x, y, z...]
                geometry.push(mesh.positions[3 * i]);
                geometry.push(mesh.positions[3 * i + 1]);
                geometry.push(mesh.positions[3 * i + 2]);
            }
        }
        
        // Retourne la géométrie + l'ADN des matériaux
        (geometry, materials)
    }
}