use std::path::PathBuf;
use walkdir::WalkDir;

/// Finds all .dart files in the given directory, excluding .g.dart files.
pub fn find_dart_files(root: &str) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok()) // Ignore files we don't have permission to read
        .filter(|e| {
            let path = e.path();

            // Check if it's a file and has .dart extension
            let is_dart = path.is_file() && path.extension().map_or(false, |ext| ext == "dart");

            // Exclude generated files (e.g., model.g.dart)
            let is_generated = path
                .file_stem()
                .map_or(false, |s| s.to_string_lossy().ends_with(".g"));

            is_dart && !is_generated
        })
        .map(|e| e.into_path())
        .collect()
}

/// Finds all generated .g.dart files in the given directory.
pub fn find_generated_files(root: &str) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok()) // Ignore files we don't have permission to read
        .filter(|e| {
            let path = e.path();

            // Check if it's a file and has .dart extension
            let is_dart = path.is_file() && path.extension().map_or(false, |ext| ext == "dart");

            // Generated files (e.g., model.g.dart)
            let is_generated = path
                .file_stem()
                .map_or(false, |s| s.to_string_lossy().ends_with(".g"));

            is_dart && is_generated
        })
        .map(|e| e.into_path())
        .collect()
}
