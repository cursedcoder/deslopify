use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::language::Language;
use super::{FileEntry, LanguageBreakdown};

pub fn compute_language_breakdown(files: &[FileEntry]) -> Vec<LanguageBreakdown> {
    let mut map: HashMap<Language, (usize, usize, u64)> = HashMap::new();

    for file in files {
        let entry = map.entry(file.language).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += file.line_count;
        entry.2 += file.size_bytes;
    }

    let mut breakdown: Vec<LanguageBreakdown> = map
        .into_iter()
        .map(
            |(language, (file_count, total_lines, total_bytes))| LanguageBreakdown {
                language,
                file_count,
                total_lines,
                total_bytes,
            },
        )
        .collect();

    breakdown.sort_by(|a, b| b.total_lines.cmp(&a.total_lines));
    breakdown
}

pub fn is_test_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if path_str.contains("/test/")
        || path_str.contains("/tests/")
        || path_str.contains("/spec/")
        || path_str.contains("/specs/")
        || path_str.contains("/__tests__/")
    {
        return true;
    }

    file_name.starts_with("test_")
        || file_name.ends_with("_test")
        || file_name.ends_with("_spec")
        || file_name.ends_with(".test")
        || file_name.ends_with(".spec")
        || file_name == "conftest"
}

pub fn compute_dir_stats(paths: &[PathBuf]) -> (usize, usize) {
    let mut max_depth: usize = 0;
    let mut top_level_dirs: usize = 0;

    for base_path in paths {
        let canonical = base_path
            .canonicalize()
            .unwrap_or_else(|_| base_path.clone());

        if let Ok(entries) = std::fs::read_dir(&canonical) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    if !dir_name.starts_with('.') {
                        top_level_dirs += 1;
                    }
                }
            }
        }

        compute_depth_recursive(&canonical, 0, &mut max_depth);
    }

    (max_depth, top_level_dirs)
}

fn compute_depth_recursive(current: &Path, depth: usize, max_depth: &mut usize) {
    if depth > *max_depth {
        *max_depth = depth;
    }

    if depth > 20 {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') && name != "node_modules" && name != "target" {
                    compute_depth_recursive(&path, depth + 1, max_depth);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::language::Language;
    use crate::scanner::FileEntry;

    fn make_file(path: &str, lang: Language, lines: usize) -> FileEntry {
        let content = "x\n".repeat(lines);
        FileEntry {
            path: PathBuf::from(path),
            language: lang,
            size_bytes: content.len() as u64,
            line_count: lines,
            content,
        }
    }

    #[test]
    fn test_file_detection_by_directory() {
        assert!(is_test_file(Path::new("src/tests/foo.py")));
        assert!(is_test_file(Path::new("src/test/foo.py")));
        assert!(is_test_file(Path::new("src/__tests__/foo.js")));
        assert!(is_test_file(Path::new("spec/models/user_spec.rb")));
    }

    #[test]
    fn test_file_detection_by_name() {
        assert!(is_test_file(Path::new("test_utils.py")));
        assert!(is_test_file(Path::new("utils_test.go")));
        assert!(is_test_file(Path::new("utils_spec.rb")));
        assert!(is_test_file(Path::new("conftest.py")));
    }

    #[test]
    fn non_test_files() {
        assert!(!is_test_file(Path::new("src/main.py")));
        assert!(!is_test_file(Path::new("lib/utils.rs")));
        assert!(!is_test_file(Path::new("app.tsx")));
        assert!(!is_test_file(Path::new("testing_helpers.py")));
    }

    #[test]
    fn language_breakdown_counts() {
        let files = vec![
            make_file("a.py", Language::Python, 100),
            make_file("b.py", Language::Python, 200),
            make_file("c.rs", Language::Rust, 50),
        ];
        let breakdown = compute_language_breakdown(&files);

        assert_eq!(breakdown.len(), 2);
        // Sorted by total_lines descending, so Python first
        assert_eq!(breakdown[0].language, Language::Python);
        assert_eq!(breakdown[0].file_count, 2);
        assert_eq!(breakdown[0].total_lines, 300);
        assert_eq!(breakdown[1].language, Language::Rust);
        assert_eq!(breakdown[1].file_count, 1);
        assert_eq!(breakdown[1].total_lines, 50);
    }

    #[test]
    fn language_breakdown_empty() {
        let breakdown = compute_language_breakdown(&[]);
        assert!(breakdown.is_empty());
    }
}
