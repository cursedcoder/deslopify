use std::collections::{HashMap, HashSet};

use crate::scanner::ScanResult;

use super::FunctionInfo;

/// Function names that are structural/idiomatic and will naturally appear in many files.
/// Flagging these as collisions adds noise.
const STRUCTURAL_NAMES: &[&str] = &[
    // PHP/JS/TS constructors
    "__construct",
    "constructor",
    // Python dunder methods
    "__init__",
    "__str__",
    "__repr__",
    "__eq__",
    "__hash__",
    "__len__",
    "__iter__",
    "__next__",
    "__enter__",
    "__exit__",
    "__getattr__",
    "__setattr__",
    "__delattr__",
    "__get__",
    "__set__",
    "__delete__",
    // Go package initialization
    "init",
    // Test framework fixtures
    "setUp",
    "tearDown",
    "setUpClass",
    "tearDownClass",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    // Common interface methods
    "toString",
    "toJSON",
    "valueOf",
    "clone",
    "equals",
    "hashCode",
    "compareTo",
    // Plugin/trait method names (intentional polymorphism, not confusion)
    "execute",
    "run",
    "handle",
    "invoke",
    "apply",
    // Rust trait impls
    "fmt",
    "from",
    "into",
    "default",
    "new",
    "drop",
];

const GENERIC_NAMES: &[&str] = &[
    "utils",
    "helpers",
    "common",
    "misc",
    "data",
    "handler",
    "process",
    "base",
    "core",
    "manager",
    "service",
    "index",
    "main",
    "lib",
    "mod",
    "init",
    "config",
    "constants",
    "types",
];

#[derive(Debug)]
pub struct SearchabilityStats {
    pub duplicate_filenames: usize,
    pub worst_duplicate_filename: Option<(String, usize)>,
    pub function_name_collisions: usize,
    pub worst_collision: Option<(String, usize)>,
    pub generic_name_count: usize,
}

pub fn analyze(scan: &ScanResult, functions: &[FunctionInfo]) -> SearchabilityStats {
    let (duplicate_filenames, worst_duplicate_filename) = count_duplicate_filenames(scan);
    let (function_name_collisions, worst_collision) = count_function_collisions(functions);
    let generic_name_count = count_generic_names(scan, functions);

    SearchabilityStats {
        duplicate_filenames,
        worst_duplicate_filename,
        function_name_collisions,
        worst_collision,
        generic_name_count,
    }
}

fn count_duplicate_filenames(scan: &ScanResult) -> (usize, Option<(String, usize)>) {
    let mut name_counts: HashMap<String, usize> = HashMap::new();

    for file in &scan.files {
        let basename = file
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        *name_counts.entry(basename).or_insert(0) += 1;
    }

    let mut duplicate_count = 0;
    let mut worst: Option<(String, usize)> = None;

    for (name, count) in &name_counts {
        if *count > 1 {
            duplicate_count += 1;
            match &worst {
                Some((_, prev_count)) if count > prev_count => {
                    worst = Some((name.clone(), *count));
                }
                None => {
                    worst = Some((name.clone(), *count));
                }
                _ => {}
            }
        }
    }

    (duplicate_count, worst)
}

fn count_function_collisions(functions: &[FunctionInfo]) -> (usize, Option<(String, usize)>) {
    let mut name_to_files: HashMap<&str, HashSet<&std::path::Path>> = HashMap::new();

    for func in functions {
        if func.name == "<anonymous>" || STRUCTURAL_NAMES.contains(&func.name.as_str()) {
            continue;
        }
        name_to_files
            .entry(&func.name)
            .or_default()
            .insert(&func.file);
    }

    let mut collision_count = 0;
    let mut worst: Option<(String, usize)> = None;

    for (name, files) in &name_to_files {
        let file_count = files.len();
        if file_count >= 3 {
            collision_count += 1;
            match &worst {
                Some((_, prev)) if file_count > *prev => {
                    worst = Some((name.to_string(), file_count));
                }
                None => {
                    worst = Some((name.to_string(), file_count));
                }
                _ => {}
            }
        }
    }

    (collision_count, worst)
}

fn count_generic_names(scan: &ScanResult, functions: &[FunctionInfo]) -> usize {
    let mut count = 0;

    for file in &scan.files {
        let stem = file
            .path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if is_generic_name(&stem) {
            count += 1;
        }
    }

    for func in functions {
        let lower = func.name.to_lowercase();
        if is_generic_name(&lower) {
            count += 1;
        }
    }

    count
}

fn is_generic_name(name: &str) -> bool {
    // Exact match on the whole name (e.g., "utils" but not "paymentUtils")
    if GENERIC_NAMES.contains(&name) {
        return true;
    }
    // Also catch names that ARE just a generic word with a file-type suffix stripped
    // (e.g., "helpers" from "helpers.py") — already handled by file_stem above.
    // But don't flag compound names like "payment_utils" or "apiHandler".
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::language::Language;
    use crate::scanner::FileEntry;
    use std::path::PathBuf;

    fn make_file(path: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            content: String::new(),
            line_count: 10,
            size_bytes: 100,
            language: Language::Python,
        }
    }

    fn make_func(name: &str, file: &str) -> FunctionInfo {
        FunctionInfo {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            line_count: 10,
            cyclomatic_complexity: 1,
            max_nesting: 0,
        }
    }

    #[test]
    fn no_duplicates() {
        let scan = ScanResult {
            files: vec![make_file("src/foo.py"), make_file("src/bar.py")],
            language_breakdown: vec![],
            configs: vec![],
            quality: crate::scanner::quality::QualitySignals::default(),
            git_activity: None,
            total_files: 2,
            total_lines: 20,
            total_bytes: 200,
            test_file_count: 0,
            source_file_count: 2,
            max_file_lines: 10,
            avg_file_lines: 10,
            max_dir_depth: 2,
            top_level_dirs: 1,
        };
        let (count, worst) = count_duplicate_filenames(&scan);
        assert_eq!(count, 0);
        assert!(worst.is_none());
    }

    #[test]
    fn duplicates_counted() {
        let scan = ScanResult {
            files: vec![
                make_file("src/api/index.ts"),
                make_file("src/web/index.ts"),
                make_file("src/cli/index.ts"),
                make_file("src/utils.py"),
            ],
            language_breakdown: vec![],
            configs: vec![],
            quality: crate::scanner::quality::QualitySignals::default(),
            git_activity: None,
            total_files: 4,
            total_lines: 40,
            total_bytes: 400,
            test_file_count: 0,
            source_file_count: 4,
            max_file_lines: 10,
            avg_file_lines: 10,
            max_dir_depth: 3,
            top_level_dirs: 1,
        };
        let (count, worst) = count_duplicate_filenames(&scan);
        assert_eq!(count, 1);
        assert_eq!(worst, Some(("index.ts".to_string(), 3)));
    }

    #[test]
    fn function_collision_threshold() {
        let funcs = vec![
            make_func("render", "a.py"),
            make_func("render", "b.py"),
            make_func("render", "c.py"),
            make_func("handle", "a.py"),
            make_func("handle", "b.py"),
        ];
        let (count, worst) = count_function_collisions(&funcs);
        assert_eq!(count, 1, "only 'render' is in 3+ files");
        assert_eq!(worst, Some(("render".to_string(), 3)));
    }

    #[test]
    fn two_files_is_not_collision() {
        let funcs = vec![make_func("process", "a.py"), make_func("process", "b.py")];
        let (count, _) = count_function_collisions(&funcs);
        assert_eq!(count, 0);
    }

    #[test]
    fn generic_name_exact_match() {
        assert!(is_generic_name("utils"));
        assert!(is_generic_name("helpers"));
        assert!(is_generic_name("common"));
    }

    #[test]
    fn compound_name_not_generic() {
        assert!(!is_generic_name("payment_utils"));
        assert!(!is_generic_name("apihandler"));
        assert!(!is_generic_name("user_service_test"));
    }

    #[test]
    fn empty_input_clean() {
        let scan = ScanResult {
            files: vec![],
            language_breakdown: vec![],
            configs: vec![],
            quality: crate::scanner::quality::QualitySignals::default(),
            git_activity: None,
            total_files: 0,
            total_lines: 0,
            total_bytes: 0,
            test_file_count: 0,
            source_file_count: 0,
            max_file_lines: 0,
            avg_file_lines: 0,
            max_dir_depth: 0,
            top_level_dirs: 0,
        };
        let stats = analyze(&scan, &[]);
        assert_eq!(stats.duplicate_filenames, 0);
        assert_eq!(stats.function_name_collisions, 0);
        assert_eq!(stats.generic_name_count, 0);
    }

    #[test]
    fn anonymous_functions_skipped() {
        let funcs = vec![
            make_func("<anonymous>", "a.py"),
            make_func("<anonymous>", "b.py"),
            make_func("<anonymous>", "c.py"),
        ];
        let (count, _) = count_function_collisions(&funcs);
        assert_eq!(count, 0);
    }

    #[test]
    fn structural_names_skipped() {
        let funcs = vec![
            make_func("__construct", "a.php"),
            make_func("__construct", "b.php"),
            make_func("__construct", "c.php"),
            make_func("__init__", "a.py"),
            make_func("__init__", "b.py"),
            make_func("__init__", "c.py"),
            make_func("init", "a.go"),
            make_func("init", "b.go"),
            make_func("init", "c.go"),
            make_func("constructor", "a.ts"),
            make_func("constructor", "b.ts"),
            make_func("constructor", "c.ts"),
            make_func("setUp", "test_a.py"),
            make_func("setUp", "test_b.py"),
            make_func("setUp", "test_c.py"),
        ];
        let (count, _) = count_function_collisions(&funcs);
        assert_eq!(count, 0, "Structural names should not count as collisions");
    }

    #[test]
    fn plugin_trait_method_names_skipped() {
        // execute/run/handle across many files = intentional polymorphism (e.g. Plugin trait), not confusion
        let funcs = vec![
            make_func("execute", "plugin_a.rs"),
            make_func("execute", "plugin_b.rs"),
            make_func("execute", "plugin_c.rs"),
            make_func("run", "task_a.ts"),
            make_func("run", "task_b.ts"),
            make_func("run", "task_c.ts"),
        ];
        let (count, _) = count_function_collisions(&funcs);
        assert_eq!(count, 0, "Plugin/trait method names should not count as collisions");
    }
}
