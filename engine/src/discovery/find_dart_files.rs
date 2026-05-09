use std::path::PathBuf;
use walkdir::WalkDir;

pub fn find_dart_files(root: &str) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();

            let is_dart = path.is_file() && path.extension().map_or(false, |ext| ext == "dart");
            let is_generated = path
                .file_stem()
                .map_or(false, |s| s.to_string_lossy().ends_with(".g"));

            is_dart && !is_generated
        })
        .map(|e| e.into_path())
        .collect()
}

pub fn find_generated_files(root: &str) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();

            let is_dart = path.is_file() && path.extension().map_or(false, |ext| ext == "dart");
            let is_generated = path
                .file_stem()
                .map_or(false, |s| s.to_string_lossy().ends_with(".g"));

            is_dart && is_generated
        })
        .map(|e| e.into_path())
        .collect()
}
