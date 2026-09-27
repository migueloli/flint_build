use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn is_dart_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "dart")
}

/// Whether the path names a `.g.dart` file. Only the name is checked, so this also works for paths that
/// no longer exist (e.g. watcher events for deleted files).
pub fn is_generated_file(path: &Path) -> bool {
    is_dart_file(path)
        && path
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().ends_with(".g"))
}

fn walk_dart_files(root: impl AsRef<Path>, generated: bool) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file() && is_dart_file(path) && is_generated_file(path) == generated
        })
        .map(|e| e.into_path())
        .collect()
}

pub fn find_dart_files(root: impl AsRef<Path>) -> Vec<PathBuf> {
    walk_dart_files(root, false)
}

pub fn find_generated_files(root: impl AsRef<Path>) -> Vec<PathBuf> {
    walk_dart_files(root, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_dart_and_generated_files() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path();

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

        assert!(is_generated_file(Path::new("lib/deleted.g.dart")));
        assert!(!is_generated_file(Path::new("lib/user.dart")));
    }
}
