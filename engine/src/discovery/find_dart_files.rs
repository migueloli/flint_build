use std::path::PathBuf;
use walkdir::WalkDir;

pub fn find_dart_files(root: &str) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();

            let is_dart = path.is_file() && path.extension().is_some_and(|ext| ext == "dart");
            let is_generated = path
                .file_stem()
                .is_some_and(|s| s.to_string_lossy().ends_with(".g"));

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

            let is_dart = path.is_file() && path.extension().is_some_and(|ext| ext == "dart");
            let is_generated = path
                .file_stem()
                .is_some_and(|s| s.to_string_lossy().ends_with(".g"));

            is_dart && is_generated
        })
        .map(|e| e.into_path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_dart_and_generated_files() {
        let temp_dir = std::env::temp_dir().join("flint_discovery_test");
        fs::create_dir_all(&temp_dir).unwrap();

        fs::write(temp_dir.join("main.dart"), "").unwrap();
        fs::write(temp_dir.join("user.model.dart"), "").unwrap();
        fs::write(temp_dir.join("user.model.g.dart"), "").unwrap();
        fs::write(temp_dir.join("README.md"), "").unwrap();

        let temp_dir_str = temp_dir.to_str().unwrap();

        let dart_files = find_dart_files(temp_dir_str);
        assert_eq!(dart_files.len(), 2);

        let filenames: Vec<String> = dart_files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(filenames.contains(&"main.dart".to_string()));
        assert!(filenames.contains(&"user.model.dart".to_string()));
        assert!(!filenames.contains(&"user.model.g.dart".to_string()));

        let generated_files = find_generated_files(temp_dir_str);
        assert_eq!(generated_files.len(), 1);
        assert_eq!(
            generated_files[0].file_name().unwrap().to_string_lossy(),
            "user.model.g.dart"
        );

        fs::remove_dir_all(temp_dir).unwrap();
    }
}
