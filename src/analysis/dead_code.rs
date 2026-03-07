use std::collections::{HashMap, HashSet};

use super::{FunctionInfo, ImportInfo};
use crate::scanner::ScanResult;

pub struct DeadCodeStats {
    pub unreferenced_function_count: usize,
    pub unreferenced_lines: usize,
}

/// Common method name prefixes that are almost always framework hooks,
/// interface implementations, or lifecycle methods. These are called by
/// frameworks via IoC, not by user code.
const FRAMEWORK_PREFIXES: &[&str] = &[
    "get", "set", "is", "has", "on", "handle", "configure", "build", "create", "update",
    "delete", "remove", "find", "to", "from", "supports", "execute", "invoke", "process",
    "render", "validate", "resolve", "load", "register", "subscribe", "provide", "apply",
    "normalize", "denormalize", "transform", "reverse", "before", "after", "pre", "post",
    "can", "should", "will", "do", "make", "parse", "format", "serialize", "deserialize",
    "map", "reduce", "filter", "convert", "run", "start", "stop", "up", "down", "boot",
];

/// Detect functions that are likely truly dead — defined but never referenced anywhere.
///
/// Conservative approach: only flags functions that are:
/// - In files that NO other file imports or references (isolated files)
/// - Have long, specific names (not short framework hooks)
/// - Are substantial in size (>= 15 lines)
/// - Don't match common framework method patterns
pub fn detect_dead_code(
    functions: &[FunctionInfo],
    imports: &[ImportInfo],
    scan: &ScanResult,
) -> DeadCodeStats {
    if functions.is_empty() {
        return DeadCodeStats {
            unreferenced_function_count: 0,
            unreferenced_lines: 0,
        };
    }

    // Step 1: determine which files are "connected" (imported/referenced by other files)
    let connected_files = find_connected_files(imports, scan);

    // Step 2: build a set of all names that appear in import paths
    let import_names: HashSet<&str> = imports
        .iter()
        .flat_map(|imp| {
            imp.module_path
                .split(['/', '.', ':', '\\', ',', ' '])
                .filter(|s| !s.is_empty())
        })
        .collect();

    let mut unreferenced_count = 0usize;
    let mut unreferenced_lines = 0usize;
    let mut seen_names: HashSet<&str> = HashSet::new();

    for func in functions {
        if should_skip(func) {
            continue;
        }

        // If name already seen in another file, it's cross-referenced
        if !seen_names.insert(&func.name) {
            continue;
        }

        // If the file containing this function is imported by any other file, skip.
        // Framework-wired classes are always imported even if individual methods aren't.
        if connected_files.contains(&func.file) {
            continue;
        }

        if import_names.contains(func.name.as_str()) {
            continue;
        }

        let referenced_elsewhere = scan.files.iter().any(|f| {
            f.path != func.file && f.content.contains(&func.name)
        });

        if !referenced_elsewhere {
            unreferenced_count += 1;
            unreferenced_lines += func.line_count;
        }
    }

    DeadCodeStats {
        unreferenced_function_count: unreferenced_count,
        unreferenced_lines,
    }
}

/// A file is "connected" if its filename (stem) appears in any import path from
/// another file, OR if any other file's content contains the filename stem.
fn find_connected_files(
    imports: &[ImportInfo],
    scan: &ScanResult,
) -> HashSet<std::path::PathBuf> {
    let mut connected = HashSet::new();

    // Map each file to its stem for matching
    let file_stems: HashMap<&std::path::Path, String> = scan
        .files
        .iter()
        .filter_map(|f| {
            let stem = f.path.file_stem()?.to_string_lossy().to_string();
            Some((f.path.as_path(), stem))
        })
        .collect();

    // A file is connected if its stem appears in any import from a different file
    for imp in imports {
        for (path, stem) in &file_stems {
            if imp.file.as_path() != *path && imp.module_path.contains(stem.as_str()) {
                connected.insert(path.to_path_buf());
            }
        }
    }

    // Also connected if its stem appears as a word in another file's content.
    // Use word-boundary matching to avoid substring false positives
    // (e.g., "legacy" matching inside "calculate_legacy_discount").
    for (path, stem) in &file_stems {
        if stem.len() < 4 {
            continue;
        }
        let referenced = scan.files.iter().any(|f| {
            if f.path.as_path() == *path {
                return false;
            }
            contains_as_word(&f.content, stem)
        });
        if referenced {
            connected.insert(path.to_path_buf());
        }
    }

    connected
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn contains_as_word(haystack: &str, needle: &str) -> bool {
    let needle_bytes = needle.as_bytes();
    let hay_bytes = haystack.as_bytes();
    if needle_bytes.len() > hay_bytes.len() {
        return false;
    }
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let abs = start + pos;
        let before_ok = abs == 0 || !is_ident_char(hay_bytes[abs - 1]);
        let after_pos = abs + needle_bytes.len();
        let after_ok = after_pos >= hay_bytes.len() || !is_ident_char(hay_bytes[after_pos]);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

fn should_skip(func: &FunctionInfo) -> bool {
    if func.name == "<anonymous>" {
        return true;
    }

    // Skip test functions
    if func.name.starts_with("test_")
        || func.name.starts_with("Test")
        || func.name.starts_with("test")
    {
        return true;
    }

    // Skip small functions — these are usually getters/setters/delegators
    if func.line_count < 15 {
        return true;
    }

    // Skip short names — too likely to be framework hooks or interface methods
    if func.name.len() < 8 {
        return true;
    }

    // Skip PHP/Python magic methods
    if func.name.starts_with("__") {
        return true;
    }

    // Skip names that start with common framework hook prefixes
    let lower = func.name.to_lowercase();
    for prefix in FRAMEWORK_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::language::Language;
    use crate::scanner::FileEntry;
    use std::path::PathBuf;

    fn make_func(name: &str, file: &str, lines: usize) -> FunctionInfo {
        FunctionInfo {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            line_count: lines,
            cyclomatic_complexity: 1,
            max_nesting: 0,
        }
    }

    fn make_file(path: &str, content: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            content: content.to_string(),
            line_count: content.lines().count(),
            size_bytes: content.len() as u64,
            language: Language::Python,
        }
    }

    fn make_scan(files: Vec<FileEntry>) -> ScanResult {
        let total_files = files.len();
        let total_lines: usize = files.iter().map(|f| f.line_count).sum();
        let total_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();
        ScanResult {
            files,
            language_breakdown: vec![],
            configs: vec![],
            quality: crate::scanner::quality::QualitySignals::default(),
            git_activity: None,
            total_files,
            total_lines,
            total_bytes,
            test_file_count: 0,
            source_file_count: total_files,
            max_file_lines: 100,
            avg_file_lines: 50,
            max_dir_depth: 3,
            top_level_dirs: 1,
        }
    }

    #[test]
    fn referenced_functions_not_flagged() {
        let funcs = vec![
            make_func("calculate_legacy_discount", "src/data.py", 20),
            make_func("normalize_address_format", "src/validator.py", 25),
        ];
        let scan = make_scan(vec![
            make_file("src/data.py", "def calculate_legacy_discount(): pass"),
            make_file("src/validator.py", "def normalize_address_format(): pass"),
            make_file("src/main.py", "from data import calculate_legacy_discount\nnormalize_address_format()"),
        ]);
        let stats = detect_dead_code(&funcs, &[], &scan);
        assert_eq!(stats.unreferenced_function_count, 0);
    }

    #[test]
    fn unreferenced_in_isolated_file() {
        let funcs = vec![
            make_func("calculate_legacy_discount", "src/data.py", 20),
            make_func("old_migration_helper", "src/legacy.py", 30),
        ];
        let scan = make_scan(vec![
            make_file("src/data.py", "def calculate_legacy_discount(): pass"),
            make_file("src/legacy.py", "def old_migration_helper(): pass"),
            make_file("src/main.py", "from data import calculate_legacy_discount"),
        ]);
        let stats = detect_dead_code(&funcs, &[], &scan);
        assert_eq!(stats.unreferenced_function_count, 1);
        assert_eq!(stats.unreferenced_lines, 30);
    }

    #[test]
    fn connected_file_functions_not_flagged() {
        // Even if a specific method isn't referenced, if the FILE is imported,
        // its methods are considered alive (framework IoC pattern)
        let funcs = vec![
            make_func("specific_internal_method", "src/UserService.py", 20),
        ];
        let imports = vec![super::super::ImportInfo {
            file: PathBuf::from("src/main.py"),
            module_path: "src.UserService".to_string(),
            line: 1,
        }];
        let scan = make_scan(vec![
            make_file("src/UserService.py", "class UserService:\n    def specific_internal_method(): pass"),
            make_file("src/main.py", "from src.UserService import UserService"),
        ]);
        let stats = detect_dead_code(&funcs, &imports, &scan);
        assert_eq!(stats.unreferenced_function_count, 0);
    }

    #[test]
    fn framework_hooks_skipped() {
        let funcs = vec![
            make_func("configureOptions", "src/form.py", 20),
            make_func("buildForm", "src/form.py", 30),
            make_func("handleRequest", "src/controller.py", 25),
            make_func("supports", "src/voter.py", 15),
            make_func("execute", "src/command.py", 40),
        ];
        let scan = make_scan(vec![
            make_file("src/form.py", ""),
            make_file("src/controller.py", ""),
            make_file("src/voter.py", ""),
            make_file("src/command.py", ""),
        ]);
        let stats = detect_dead_code(&funcs, &[], &scan);
        assert_eq!(stats.unreferenced_function_count, 0);
    }

    #[test]
    fn short_names_skipped() {
        let funcs = vec![
            make_func("index", "src/a.py", 30),
            make_func("edit", "src/a.py", 30),
            make_func("show", "src/a.py", 30),
        ];
        let scan = make_scan(vec![make_file("src/a.py", "")]);
        let stats = detect_dead_code(&funcs, &[], &scan);
        assert_eq!(stats.unreferenced_function_count, 0);
    }

    #[test]
    fn small_functions_skipped() {
        let funcs = vec![make_func("some_unused_helper", "src/a.py", 10)];
        let scan = make_scan(vec![make_file("src/a.py", "")]);
        let stats = detect_dead_code(&funcs, &[], &scan);
        assert_eq!(stats.unreferenced_function_count, 0);
    }

    #[test]
    fn empty_input() {
        let scan = make_scan(vec![]);
        let stats = detect_dead_code(&[], &[], &scan);
        assert_eq!(stats.unreferenced_function_count, 0);
        assert_eq!(stats.unreferenced_lines, 0);
    }
}
