use crate::analysis::AnalysisResult;
use crate::scanner::{ConfigKind, ScanResult};
use crate::scoring::ScoreResult;

use super::Recommendation;

pub fn generate_all(
    _score: &ScoreResult,
    scan: &ScanResult,
    analysis: &AnalysisResult,
) -> Vec<Recommendation> {
    let mut recs = Vec::new();

    split_large_files(scan, analysis, &mut recs);
    add_tests(scan, &mut recs);
    add_formatter_linter(scan, &mut recs);
    add_documentation(scan, &mut recs);
    reduce_complexity(analysis, &mut recs);
    resolve_circular_deps(analysis, &mut recs);
    remove_anti_patterns(analysis, &mut recs);
    add_type_checking(scan, &mut recs);
    add_ci(scan, &mut recs);
    reduce_global_mutables(analysis, &mut recs);
    extract_libraries(scan, &mut recs);

    recs
}

fn split_large_files(scan: &ScanResult, analysis: &AnalysisResult, recs: &mut Vec<Recommendation>) {
    let mut large_files: Vec<_> = scan
        .files
        .iter()
        .filter(|f| f.line_count > 500 && f.language.is_source_code())
        .collect();
    large_files.sort_by(|a, b| b.line_count.cmp(&a.line_count));

    for file in large_files.iter().take(3) {
        let fn_count = analysis
            .functions
            .iter()
            .filter(|f| f.file == file.path)
            .count();

        let reduction = if file.line_count > 1000 { 8 } else { 4 };

        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: reduction,
            title: format!("Split {} ({} lines, {} functions)", file.path.display(), file.line_count, fn_count),
            target: file.path.display().to_string(),
            action: "Break this file into smaller modules. Group related functions into separate files of 200-300 lines each. Re-export the public API from a central module.".to_string(),
            evidence: format!("{} lines, {} functions", file.line_count, fn_count),
        });
    }
}

fn add_tests(scan: &ScanResult, recs: &mut Vec<Recommendation>) {
    let test_ratio = if scan.source_file_count > 0 {
        scan.test_file_count as f64 / scan.source_file_count as f64
    } else {
        0.0
    };

    if test_ratio < 0.2 {
        let reduction = if test_ratio < 0.05 { 6 } else { 3 };
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: reduction,
            title: format!(
                "Add tests ({} test files for {} source files, ratio {:.2})",
                scan.test_file_count, scan.source_file_count, test_ratio
            ),
            target: "project-wide".to_string(),
            action: "Add unit tests for core modules. Prioritize modules with the most imports (highest fan-in) as they have the largest blast radius. Each source directory should have a corresponding test file.".to_string(),
            evidence: format!("test ratio: {:.2}", test_ratio),
        });
    }
}

fn add_formatter_linter(scan: &ScanResult, recs: &mut Vec<Recommendation>) {
    let has_formatter = scan.configs.iter().any(|c| c.kind == ConfigKind::Formatter);
    let has_linter = scan.configs.iter().any(|c| c.kind == ConfigKind::Linter);

    if !has_formatter {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 2,
            title: "Configure a code formatter".to_string(),
            target: "project config".to_string(),
            action: "Add a formatter config (e.g., Prettier for JS/TS, Black/Ruff for Python, rustfmt for Rust). Run it on the entire codebase. Consistent formatting eliminates style noise for LLM agents.".to_string(),
            evidence: "no formatter configuration found".to_string(),
        });
    }

    if !has_linter {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 2,
            title: "Configure a linter".to_string(),
            target: "project config".to_string(),
            action: "Add a linter config (e.g., ESLint for JS/TS, Ruff for Python, Clippy for Rust). Enforce consistent patterns so LLM agents can predict code conventions.".to_string(),
            evidence: "no linter configuration found".to_string(),
        });
    }
}

fn add_documentation(scan: &ScanResult, recs: &mut Vec<Recommendation>) {
    let has_readme = scan.configs.iter().any(|c| c.kind == ConfigKind::Readme);
    let has_arch = scan
        .configs
        .iter()
        .any(|c| c.kind == ConfigKind::ArchitectureDocs);

    if !has_readme {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 3,
            title: "Add a README".to_string(),
            target: "project root".to_string(),
            action: "Create a README.md with: project purpose, setup instructions, directory structure overview, and key commands (build, test, lint).".to_string(),
            evidence: "no README found".to_string(),
        });
    }

    if !has_arch && scan.top_level_dirs > 5 {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 3,
            title: format!(
                "Add architecture documentation ({} top-level directories)",
                scan.top_level_dirs
            ),
            target: "project root".to_string(),
            action: "Create an ARCHITECTURE.md or AGENTS.md explaining the module structure, data flow, and key abstractions. This dramatically reduces the context an LLM needs to navigate the codebase.".to_string(),
            evidence: format!("{} top-level directories with no architecture guide", scan.top_level_dirs),
        });
    }
}

fn reduce_complexity(analysis: &AnalysisResult, recs: &mut Vec<Recommendation>) {
    let mut complex_fns: Vec<_> = analysis
        .functions
        .iter()
        .filter(|f| f.cyclomatic_complexity > 15 || f.line_count > 100)
        .collect();
    complex_fns.sort_by(|a, b| b.cyclomatic_complexity.cmp(&a.cyclomatic_complexity));

    if let Some(worst) = complex_fns.first() {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 3,
            title: format!(
                "Simplify {} (complexity {}, {} lines)",
                worst.name, worst.cyclomatic_complexity, worst.line_count
            ),
            target: format!("{}:{}", worst.file.display(), worst.start_line),
            action: format!(
                "Extract helper functions from this complex function. Target complexity < 10 and length < 50 lines. {} functions exceed complexity threshold.",
                complex_fns.len()
            ),
            evidence: format!(
                "cyclomatic complexity {}, {} lines, nesting depth {}",
                worst.cyclomatic_complexity, worst.line_count, worst.max_nesting
            ),
        });
    }
}

fn resolve_circular_deps(analysis: &AnalysisResult, recs: &mut Vec<Recommendation>) {
    use crate::analysis::imports;
    let graph = imports::compute_import_graph(&analysis.imports);

    if graph.circular_dep_count > 0 {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 4,
            title: format!("Resolve {} circular dependencies", graph.circular_dep_count),
            target: "import graph".to_string(),
            action: "Break circular imports by extracting shared types/interfaces into a separate module. Use dependency inversion: depend on abstractions, not concrete implementations.".to_string(),
            evidence: format!("{} circular dependency pairs detected", graph.circular_dep_count),
        });
    }
}

fn remove_anti_patterns(analysis: &AnalysisResult, recs: &mut Vec<Recommendation>) {
    let pattern_count = analysis.anti_patterns.len();
    if pattern_count > 10 {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 2,
            title: format!("Clean up {} anti-patterns", pattern_count),
            target: "project-wide".to_string(),
            action: "Remove debug prints, TODO placeholders, commented-out code, and bare except blocks. These add noise that LLM agents must parse through.".to_string(),
            evidence: format!("{} anti-pattern matches across the codebase", pattern_count),
        });
    }
}

fn add_type_checking(scan: &ScanResult, recs: &mut Vec<Recommendation>) {
    let has_types = scan
        .configs
        .iter()
        .any(|c| c.kind == ConfigKind::TypeChecker);

    if !has_types && scan.source_file_count > 10 {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 3,
            title: "Add type checking".to_string(),
            target: "project config".to_string(),
            action: "Configure a type checker (tsconfig.json for TS, mypy for Python, etc.). Type annotations give LLM agents critical context about function contracts without reading implementations.".to_string(),
            evidence: "no type checker configuration found".to_string(),
        });
    }
}

fn add_ci(scan: &ScanResult, recs: &mut Vec<Recommendation>) {
    let has_ci = scan.configs.iter().any(|c| c.kind == ConfigKind::CI);

    if !has_ci {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 2,
            title: "Set up CI pipeline".to_string(),
            target: "project config".to_string(),
            action: "Add a CI configuration (GitHub Actions, GitLab CI, etc.) that runs linting, type checking, and tests on every push. This gives LLM agents a fast feedback loop.".to_string(),
            evidence: "no CI configuration found".to_string(),
        });
    }
}

fn reduce_global_mutables(analysis: &AnalysisResult, recs: &mut Vec<Recommendation>) {
    if analysis.global_mutable_count > 10 {
        recs.push(Recommendation {
            priority: 0,
            estimated_reduction: 2,
            title: format!(
                "Reduce global mutable state ({} instances)",
                analysis.global_mutable_count
            ),
            target: "project-wide".to_string(),
            action: "Replace global mutable state with dependency injection or module-scoped constants. Global state makes it impossible for LLM agents to reason about side effects.".to_string(),
            evidence: format!("{} global mutable patterns detected", analysis.global_mutable_count),
        });
    }
}

fn extract_libraries(scan: &ScanResult, recs: &mut Vec<Recommendation>) {
    let git = match &scan.git_activity {
        Some(g) if g.is_git_repo && g.active_files > 0 => g,
        _ => return,
    };

    let active_pct = git.active_files as f64 / scan.total_files.max(1) as f64;

    // Only recommend when codebase is large enough AND the active surface is wide.
    // A small project where everything changes is fine — there's nothing to extract.
    // A large project where >60% of files are active has too much surface area for
    // an LLM agent; extracting stable utilities into libraries shrinks the tip.
    if scan.total_files < 50 || active_pct < 0.60 {
        return;
    }

    let hot_names: Vec<String> = git
        .hot_files
        .iter()
        .take(3)
        .map(|h| {
            h.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let reduction = if active_pct > 0.85 { 5 } else { 3 };

    recs.push(Recommendation {
        priority: 0,
        estimated_reduction: reduction,
        title: format!(
            "Extract libraries to shrink active surface ({:.0}% of files changed in {} months)",
            active_pct * 100.0,
            git.window_months
        ),
        target: "project architecture".to_string(),
        action: format!(
            "Move stable utility code into internal libraries or packages that rarely change. \
             This shrinks the \"tip of the iceberg\" — the active surface an LLM agent must \
             reason about. Focus on code that doesn't appear in the hot files list. \
             Currently {} of {} files changed recently; extracting shared helpers, data models, \
             and configuration into versioned libraries would reduce context pressure significantly.",
            git.active_files, scan.total_files
        ),
        evidence: format!(
            "{} active files ({:.0}%), {} frozen files, hottest: {}",
            git.active_files,
            active_pct * 100.0,
            git.frozen_files,
            if hot_names.is_empty() {
                "n/a".to_string()
            } else {
                hot_names.join(", ")
            }
        ),
    });
}
