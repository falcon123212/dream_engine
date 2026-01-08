use seed_architect::importer::SeedImporter;
use log::info;
use std::path::Path;

fn main() {
    // Initialisation des logs pour voir ce qu'il se passe
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    info!("🏗️ [ARCHITECT] Démarrage du protocole de baking...");

    // Chemins (Universels)
    let input_path = "assets/raw/relic_test.obj";
    let output_dir = "assets/processed";
    let output_path = "assets/processed/relic.seed";

    // Vérification de l'entrée
    if !Path::new(input_path).exists() {
        panic!("❌ Fichier source introuvable : {}", input_path);
    }

    // Création du dossier de sortie si nécessaire
    if !Path::new(output_dir).exists() {
        std::fs::create_dir_all(output_dir).expect("❌ Impossible de créer le dossier assets/processed");
    }

    // Lancement de la conversion
    info!("🔥 Baking en cours : {} -> {}", input_path, output_path);
    SeedImporter::import_and_bake(input_path, output_path);
    
    info!("✅ [ARCHITECT] Succès ! Fichier .seed prêt pour le Runtime.");
}