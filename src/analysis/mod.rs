pub mod complexity;
pub mod duplication;
pub mod imports;
pub mod naming;
pub mod patterns;
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
    pub avg_function_lines: usize,
    pub max_function_lines: usize,
    pub max_nesting_depth: usize,
    pub avg_cyclomatic_complexity: f64,
}

pub fn analyze(scan: &ScanResult) -> AnalysisResult {
    let source_files: Vec<&FileEntry> = scan
        .files
        .iter()
        .filter(|f| f.language.is_source_code())
        .collect();

    let mut all_functions = Vec::new();
    let mut all_imports = Vec::new();

    for file in &source_files {
        if file.language.has_tree_sitter_support() {
            if let Some(tree) = tree_sitter_util::parse_file(file) {
                let mut funcs = complexity::extract_functions(&tree, file);
                all_functions.append(&mut funcs);

                let mut imps = imports::extract_imports(&tree, file);
                all_imports.append(&mut imps);
            }
        }
    }

    let duplicates = duplication::find_duplicates(&source_files);
    let naming = naming::analyze_naming(&all_functions);
    let anti_patterns = patterns::detect_patterns(&source_files);
    let global_mutable_count = patterns::count_global_mutables(&source_files);

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
        avg_function_lines,
        max_function_lines,
        max_nesting_depth,
        avg_cyclomatic_complexity,
    }
}
