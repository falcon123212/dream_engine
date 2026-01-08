use bytemuck::{Pod, Zeroable};
use std::fs::File;
use std::io::Write;
use crate::{SeedFileHeader, encode_morton_3d};
use rand::Rng;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, serde::Serialize, serde::Deserialize)]
pub struct MaterialData {
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub emissive_ptr: u64,
    pub roughness: f32,
    pub ior: f32,
    pub _padding: [u32; 2],
}

pub struct SeedImporter;

impl SeedImporter {
    pub fn import_and_bake(path: &str, output_path: &str) {
        let (geometry, _materials) = Self::load_raw_obj(path);
        
        let mut rng = rand::thread_rng();
        
        // CORRECTION ICI :
        // 0.5 était trop grand. On passe à 0.02 pour juste "épaissir" la surface
        // sans détruire les traits du visage de Suzanne.
        let dispersion = 0.02; 

        // 1. Conversion et Injection du Bruit
        let mut vertices: Vec<[f32; 6]> = Vec::with_capacity(geometry.len() / 6);
        for i in (0..geometry.len()).step_by(6) {
            if i + 5 < geometry.len() {
                // Jitter très faible pour garder la forme
                let jitter_x = rng.gen_range(-dispersion..dispersion);
                let jitter_y = rng.gen_range(-dispersion..dispersion);
                let jitter_z = rng.gen_range(-dispersion..dispersion);

                vertices.push([
                    geometry[i]   + jitter_x, 
                    geometry[i+1] + jitter_y, 
                    geometry[i+2] + jitter_z, 
                    geometry[i+3], geometry[i+4], geometry[i+5]
                ]);
            }
        }   

        // 2. Tri Morton (Optimisation Cache GPU)
        vertices.sort_by_key(|v| {
            let x = ((v[0] + 512.0) * 100.0) as u32;
            let y = ((v[1] + 512.0) * 100.0) as u32;
            let z = ((v[2] + 512.0) * 100.0) as u32;
            encode_morton_3d(x, y, z)
        });

        // 3. Écriture du Fichier .SEED
        let mut file = File::create(output_path).expect("❌ Impossible de créer le fichier .seed");
        
        // Header
        let header = SeedFileHeader {
            magic: *b"SEED",
            version: 2030,
            vertex_count: vertices.len() as u64,
            index_count: 0,
            bvh_offset: 0, 
            material_ptr: 0, 
        };
        
        let header_bytes = bincode::serialize(&header).unwrap();
        file.write_all(&header_bytes).unwrap();

        // Data (Positions + Normales)
        let geo_bytes = bytemuck::cast_slice(&vertices);
        file.write_all(geo_bytes).unwrap();
    }

    fn load_raw_obj(path: &str) -> (Vec<f32>, Vec<MaterialData>) {
        let load_options = tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ..Default::default()
        };

        // Charge le modèle
        let (models, materials_result) = tobj::load_obj(path, &load_options)
            .expect("❌ Erreur Ingestion OBJ (Vérifiez le chemin)");

        // Gestion Placeholder des matériaux
        let materials = match materials_result {
            Ok(mats) => mats.into_iter().map(|m| MaterialData {
                base_color: m.diffuse.unwrap_or([1.0, 0.84, 0.0]),
                metallic: 1.0,
                emissive_ptr: 0,
                roughness: 0.3,
                ior: m.optical_density.unwrap_or(1.45),
                _padding: [0, 0],
            }).collect(),
            Err(_) => vec![MaterialData {
                base_color: [1.0, 0.84, 0.0],
                metallic: 1.0,
                emissive_ptr: 0,
                roughness: 0.3,
                ior: 1.45,
                _padding: [0, 0],
            }],
        };

        let mut geometry = Vec::new();
        for model in models {
            let mesh = model.mesh;
            for index in mesh.indices {
                let i = index as usize;
                // Position
                geometry.push(mesh.positions[3 * i]);
                geometry.push(mesh.positions[3 * i + 1]);
                geometry.push(mesh.positions[3 * i + 2]);

                // Normale
                if !mesh.normals.is_empty() {
                    geometry.push(mesh.normals[3 * i]);
                    geometry.push(mesh.normals[3 * i + 1]);
                    geometry.push(mesh.normals[3 * i + 2]);
                } else {
                    geometry.push(0.0); geometry.push(1.0); geometry.push(0.0);
                }
            }
        }
        (geometry, materials)
    }
}