use regex::Regex;
use std::path::PathBuf;

use super::{ConfigKind, DetectedConfig, LanguageBreakdown};

#[derive(Debug)]
pub struct QualitySignals {
    pub readme_bytes: u64,
    pub readme_has_setup_instructions: bool,

    pub build_script_has_standard_targets: bool,
    pub dockerfile_has_useful_commands: bool,

    pub estimated_build_seconds: f64,

    pub lockfile_appears_fresh: bool,
}

impl Default for QualitySignals {
    fn default() -> Self {
        Self {
            readme_bytes: 0,
            readme_has_setup_instructions: false,
            build_script_has_standard_targets: false,
            dockerfile_has_useful_commands: false,
            estimated_build_seconds: 0.0,
            lockfile_appears_fresh: false,
        }
    }
}

pub fn compute(
    configs: &[DetectedConfig],
    language_breakdown: &[LanguageBreakdown],
    total_lines: usize,
) -> QualitySignals {
    let mut signals = QualitySignals::default();

    for config in configs {
        match config.kind {
            ConfigKind::Readme => check_readme(&config.path, &mut signals),
            ConfigKind::BuildScript => check_build_script(&config.path, &mut signals),
            ConfigKind::Docker => check_dockerfile(&config.path, &mut signals),
            _ => {}
        }
    }

    signals.estimated_build_seconds = estimate_build_seconds(language_breakdown, total_lines);
    signals.lockfile_appears_fresh = check_lockfile_freshness(configs);

    signals
}

fn check_readme(path: &PathBuf, signals: &mut QualitySignals) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    signals.readme_bytes = content.len() as u64;

    let lower = content.to_lowercase();
    let setup_keywords = [
        "install",
        "setup",
        "getting started",
        "quick start",
        "quickstart",
        "how to run",
        "how to build",
        "usage",
        "build & run",
        "build and run",
    ];
    let run_keywords = [
        "cargo run",
        "npm start",
        "npm run",
        "yarn start",
        "python ",
        "go run",
        "make run",
        "make build",
        "docker run",
        "docker compose",
        "./",
    ];

    let has_setup = setup_keywords.iter().any(|kw| lower.contains(kw));
    let has_run = run_keywords.iter().any(|kw| lower.contains(kw));
    signals.readme_has_setup_instructions = has_setup || has_run;
}

fn check_build_script(path: &PathBuf, signals: &mut QualitySignals) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let lower = content.to_lowercase();
    let target_re = Regex::new(r"(?m)^([\w-]+)\s*:").unwrap();
    let targets: Vec<String> = target_re
        .captures_iter(&lower)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    let standard_targets = [
        "build", "test", "run", "clean", "install", "lint", "check", "dev",
    ];
    let has_standard = standard_targets
        .iter()
        .any(|t| targets.iter().any(|found| found == t));

    if has_standard {
        signals.build_script_has_standard_targets = true;
    }

    if path.file_name().map(|n| n.to_string_lossy().to_lowercase()) == Some("justfile".into()) {
        let just_re = Regex::new(r"(?m)^([\w-]+)\s*:").unwrap();
        let just_targets: Vec<String> = just_re
            .captures_iter(&content)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_lowercase()))
            .collect();
        if standard_targets
            .iter()
            .any(|t| just_targets.iter().any(|found| found == t))
        {
            signals.build_script_has_standard_targets = true;
        }
    }
}

fn check_dockerfile(path: &PathBuf, signals: &mut QualitySignals) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if name.contains("compose") {
        signals.dockerfile_has_useful_commands = true;
        return;
    }

    if name != "dockerfile" {
        return;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let upper = content.to_uppercase();
    let has_from = upper.lines().any(|l| l.trim_start().starts_with("FROM "));
    let has_action = upper.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("RUN ") || t.starts_with("CMD ") || t.starts_with("ENTRYPOINT ")
    });

    signals.dockerfile_has_useful_commands = has_from && has_action;
}

/// Rough build-time heuristic: lines * language factor.
/// Factors are seconds-per-1000-lines for a typical incremental build.
fn estimate_build_seconds(breakdown: &[LanguageBreakdown], total_lines: usize) -> f64 {
    use super::language::Language;

    if breakdown.is_empty() || total_lines == 0 {
        return 0.0;
    }

    let dominant = &breakdown[0];
    let factor = match dominant.language {
        Language::Rust => 0.8,
        Language::Cpp | Language::C => 0.6,
        Language::Java | Language::Scala | Language::Kotlin => 0.3,
        Language::Go => 0.1,
        Language::TypeScript | Language::Tsx => 0.15,
        Language::JavaScript | Language::Jsx => 0.05,
        Language::Python | Language::Ruby | Language::Php | Language::Elixir => 0.02,
        Language::Dart | Language::Swift => 0.2,
        _ => 0.1,
    };

    (total_lines as f64 / 1000.0) * factor
}

fn check_lockfile_freshness(configs: &[DetectedConfig]) -> bool {
    let lockfiles: Vec<&DetectedConfig> = configs
        .iter()
        .filter(|c| c.kind == ConfigKind::Lockfile)
        .collect();
    let manifests: Vec<&DetectedConfig> = configs
        .iter()
        .filter(|c| c.kind == ConfigKind::DependencyManager)
        .collect();

    if lockfiles.is_empty() || manifests.is_empty() {
        return false;
    }

    let lock_mtime = lockfiles
        .iter()
        .filter_map(|c| std::fs::metadata(&c.path).ok()?.modified().ok())
        .max();
    let manifest_mtime = manifests
        .iter()
        .filter_map(|c| std::fs::metadata(&c.path).ok()?.modified().ok())
        .max();

    match (lock_mtime, manifest_mtime) {
        (Some(lock), Some(manifest)) => lock >= manifest,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::language::Language;
    use super::*;

    #[test]
    fn build_seconds_rust_project() {
        let breakdown = vec![LanguageBreakdown {
            language: Language::Rust,
            file_count: 20,
            total_lines: 5000,
            total_bytes: 150000,
        }];
        let secs = estimate_build_seconds(&breakdown, 5000);
        assert!(secs > 1.0, "Rust 5k lines should take >1s, got {:.1}", secs);
        assert!(
            secs < 20.0,
            "Rust 5k lines should take <20s, got {:.1}",
            secs
        );
    }

    #[test]
    fn build_seconds_python_project() {
        let breakdown = vec![LanguageBreakdown {
            language: Language::Python,
            file_count: 50,
            total_lines: 10000,
            total_bytes: 300000,
        }];
        let secs = estimate_build_seconds(&breakdown, 10000);
        assert!(
            secs < 1.0,
            "Python 10k lines should take <1s, got {:.1}",
            secs
        );
    }

    #[test]
    fn build_seconds_empty() {
        let secs = estimate_build_seconds(&[], 0);
        assert!((secs - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lockfile_freshness_no_files() {
        assert!(!check_lockfile_freshness(&[]));
    }
}
