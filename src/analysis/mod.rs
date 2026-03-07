pub mod complexity;
pub mod dead_code;
pub mod duplication;
pub mod imports;
pub mod layers;
pub mod naming;
pub mod patterns;
pub mod runtime;
pub mod searchability;
pub mod tree_sitter_util;

use crate::scanner::{FileEntry, ScanResult};

#[derive(Debug)]
pub struct FunctionInfo {
    pub name: String,
    pub file: std::path::PathBuf,
    pub start_line: usize,
    pub line_count: usize,
    pub cyclomatic_complexity: usize,
    pub max_nesting: usize,
}

#[derive(Debug)]
pub struct ImportInfo {
    pub file: std::path::PathBuf,
    pub module_path: String,
    #[allow(dead_code)]
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct DuplicateCluster {
    #[allow(dead_code)]
    pub hash: u64,
    pub locations: Vec<(std::path::PathBuf, usize)>,
    pub line_count: usize,
    pub similarity: f64,
}

#[derive(Debug)]
pub struct NamingStats {
    pub snake_case_count: usize,
    pub camel_case_count: usize,
    pub pascal_case_count: usize,
    pub screaming_snake_count: usize,
    pub mixed_count: usize,
}

impl NamingStats {
    pub fn total(&self) -> usize {
        self.snake_case_count
            + self.camel_case_count
            + self.pascal_case_count
            + self.screaming_snake_count
            + self.mixed_count
    }

    pub fn dominant_style_ratio(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 1.0;
        }
        let max = self
            .snake_case_count
            .max(self.camel_case_count)
            .max(self.pascal_case_count)
            .max(self.screaming_snake_count);
        max as f64 / total as f64
    }
}

#[derive(Debug)]
pub struct PatternMatch {
    #[allow(dead_code)]
    pub file: std::path::PathBuf,
    #[allow(dead_code)]
    pub line: usize,
    pub pattern_name: String,
    #[allow(dead_code)]
    pub description: String,
}

#[derive(Debug)]
pub struct AnalysisResult {
    pub functions: Vec<FunctionInfo>,
    pub imports: Vec<ImportInfo>,
    pub duplicates: Vec<DuplicateCluster>,
    pub naming: NamingStats,
    pub anti_patterns: Vec<PatternMatch>,
    pub global_mutable_count: usize,
    pub runtime_hazard_count: usize,
    pub layer_violations: usize,
    pub god_module_count: usize,
    pub avg_function_lines: usize,
    pub max_function_lines: usize,
    pub max_nesting_depth: usize,
    pub avg_cyclomatic_complexity: f64,
    pub duplicate_filename_count: usize,
    pub function_collision_count: usize,
    pub generic_name_count: usize,
    pub worst_duplicate_filename: Option<(String, usize)>,
    pub worst_function_collision: Option<(String, usize)>,
    pub unreferenced_function_count: usize,
    pub unreferenced_lines: usize,
}

fn is_likely_minified(file: &FileEntry) -> bool {
    if file.line_count == 0 {
        return false;
    }
    let avg_line_len = file.size_bytes as f64 / file.line_count as f64;
    // Single-line files over 1KB, or files with avg line length > 200 chars
    (file.line_count <= 3 && file.size_bytes > 1024) || avg_line_len > 200.0
}

pub fn analyze(scan: &ScanResult) -> AnalysisResult {
    let source_files: Vec<&FileEntry> = scan
        .files
        .iter()
        .filter(|f| f.language.is_source_code() && !is_likely_minified(f))
        .collect();

    let mut all_functions = Vec::new();
    let mut all_imports = Vec::new();
    let mut all_runtime_hazards = Vec::new();

    for file in &source_files {
        if file.language.has_tree_sitter_support() {
            if let Some(tree) = tree_sitter_util::parse_file(file) {
                let mut funcs = complexity::extract_functions(&tree, file);
                all_functions.append(&mut funcs);

                let mut imps = imports::extract_imports(&tree, file);
                all_imports.append(&mut imps);

                let mut hazards = runtime::detect_runtime_hazards(&tree, file);
                all_runtime_hazards.append(&mut hazards);
            }
        }
    }

    let duplicates = duplication::find_duplicates(&source_files);
    let naming = naming::analyze_naming(&all_functions);
    let anti_patterns = patterns::detect_patterns(&source_files);
    let global_mutable_count = patterns::count_global_mutables(&source_files);
    let runtime_hazard_count = all_runtime_hazards.len();

    let layer_analysis = layers::analyze_layers(&all_imports, scan);
    let layer_violations = layer_analysis.bidirectional_pairs.len();
    let god_module_count = layer_analysis.god_modules.len();

    let search_stats = searchability::analyze(scan, &all_functions);
    let dead = dead_code::detect_dead_code(&all_functions, &all_imports, scan);

    let avg_function_lines = if all_functions.is_empty() {
        0
    } else {
        all_functions.iter().map(|f| f.line_count).sum::<usize>() / all_functions.len()
    };
    let max_function_lines = all_functions
        .iter()
        .map(|f| f.line_count)
        .max()
        .unwrap_or(0);
    let max_nesting_depth = all_functions
        .iter()
        .map(|f| f.max_nesting)
        .max()
        .unwrap_or(0);
    let avg_cyclomatic_complexity = if all_functions.is_empty() {
        0.0
    } else {
        all_functions
            .iter()
            .map(|f| f.cyclomatic_complexity as f64)
            .sum::<f64>()
            / all_functions.len() as f64
    };

    AnalysisResult {
        functions: all_functions,
        imports: all_imports,
        duplicates,
        naming,
        anti_patterns,
        global_mutable_count,
        runtime_hazard_count,
        layer_violations,
        god_module_count,
        avg_function_lines,
        max_function_lines,
        max_nesting_depth,
        avg_cyclomatic_complexity,
        duplicate_filename_count: search_stats.duplicate_filenames,
        function_collision_count: search_stats.function_name_collisions,
        generic_name_count: search_stats.generic_name_count,
        worst_duplicate_filename: search_stats.worst_duplicate_filename,
        worst_function_collision: search_stats.worst_collision,
        unreferenced_function_count: dead.unreferenced_function_count,
        unreferenced_lines: dead.unreferenced_lines,
    }
}
