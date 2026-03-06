pub mod config;
pub mod language;
pub mod quality;
pub mod stats;
pub mod walker;

use std::path::PathBuf;

use language::Language;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub language: Language,
    pub size_bytes: u64,
    pub line_count: usize,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct LanguageBreakdown {
    pub language: Language,
    pub file_count: usize,
    pub total_lines: usize,
    pub total_bytes: u64,
}

#[derive(Debug)]
pub struct DetectedConfig {
    pub kind: ConfigKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConfigKind {
    Formatter,
    Linter,
    TypeChecker,
    TestFramework,
    CI,
    Docker,
    DependencyManager,
    Lockfile,
    BuildScript,
    EditorConfig,
    Readme,
    ArchitectureDocs,
    Contributing,
    GitIgnore,
}

impl ConfigKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Formatter => "Formatter",
            Self::Linter => "Linter",
            Self::TypeChecker => "Type Checker",
            Self::TestFramework => "Test Framework",
            Self::CI => "CI",
            Self::Docker => "Docker",
            Self::DependencyManager => "Dependency Manager",
            Self::Lockfile => "Lockfile",
            Self::BuildScript => "Build Script",
            Self::EditorConfig => "Editor Config",
            Self::Readme => "README",
            Self::ArchitectureDocs => "Architecture Docs",
            Self::Contributing => "Contributing Guide",
            Self::GitIgnore => ".gitignore",
        }
    }
}

#[derive(Debug)]
pub struct ScanResult {
    pub files: Vec<FileEntry>,
    pub language_breakdown: Vec<LanguageBreakdown>,
    pub configs: Vec<DetectedConfig>,
    pub quality: quality::QualitySignals,
    pub total_files: usize,
    pub total_lines: usize,
    pub total_bytes: u64,
    pub test_file_count: usize,
    pub source_file_count: usize,
    pub max_file_lines: usize,
    pub avg_file_lines: usize,
    pub max_dir_depth: usize,
    pub top_level_dirs: usize,
}

pub fn scan(paths: &[PathBuf], ignore_patterns: &[String]) -> ScanResult {
    let entries = walker::walk(paths, ignore_patterns);
    let files: Vec<FileEntry> = entries
        .into_iter()
        .filter_map(|path| {
            let lang = language::detect(&path);
            if lang == Language::Unknown {
                return None;
            }
            let content = std::fs::read_to_string(&path).ok()?;
            let line_count = content.lines().count();
            let size_bytes = content.len() as u64;
            Some(FileEntry {
                path,
                language: lang,
                size_bytes,
                line_count,
                content,
            })
        })
        .collect();

    let language_breakdown = stats::compute_language_breakdown(&files);
    let configs = config::detect_configs(paths);
    let total_files = files.len();
    let total_lines: usize = files.iter().map(|f| f.line_count).sum();
    let total_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();
    let max_file_lines = files.iter().map(|f| f.line_count).max().unwrap_or(0);
    let avg_file_lines = if total_files > 0 {
        total_lines / total_files
    } else {
        0
    };

    let test_file_count = files
        .iter()
        .filter(|f| stats::is_test_file(&f.path))
        .count();
    let source_file_count = total_files - test_file_count;

    let (max_dir_depth, top_level_dirs) = stats::compute_dir_stats(paths);

    let quality_signals = quality::compute(&configs, &language_breakdown, total_lines);

    ScanResult {
        files,
        language_breakdown,
        configs,
        quality: quality_signals,
        total_files,
        total_lines,
        total_bytes,
        test_file_count,
        source_file_count,
        max_file_lines,
        avg_file_lines,
        max_dir_depth,
        top_level_dirs,
    }
}
