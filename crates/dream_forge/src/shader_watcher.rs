use notify::{Watcher, RecursiveMode, Event};
use std::path::Path;
use std::sync::mpsc::Receiver;

pub fn create_shader_watcher() -> (notify::RecommendedWatcher, Receiver<notify::Result<Event>>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx).expect("❌ Watcher KO");
    
    // Vérifie que le dossier existe avant de surveiller
    let shader_path = Path::new("assets/shaders");
    if !shader_path.exists() {
        std::fs::create_dir_all(shader_path).unwrap();
    }

    watcher.watch(shader_path, RecursiveMode::Recursive).unwrap();
    
    (watcher, rx)
}