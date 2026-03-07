use crate::analysis::imports;
use crate::analysis::AnalysisResult;
use crate::scanner::{ConfigKind, ScanResult};

use super::DimensionScore;

pub fn compute_all(scan: &ScanResult, analysis: &AnalysisResult) -> Vec<DimensionScore> {
    vec![
        setup_reliability(scan),
        architecture_clarity(scan, analysis),
        coupling_blast_radius(analysis),
        style_consistency(scan, analysis),
        test_safety_net(scan),
        runtime_predictability(analysis),
        feedback_loop_speed(scan),
        documentation(scan),
        dependency_boundaries(scan),
        context_pressure(scan, analysis),
    ]
}

fn has_config(scan: &ScanResult, kind: &ConfigKind) -> bool {
    scan.configs.iter().any(|c| &c.kind == kind)
}

fn setup_reliability(scan: &ScanResult) -> DimensionScore {
    let mut rating = 5u32;
    let q = &scan.quality;

    if has_config(scan, &ConfigKind::DependencyManager) {
        rating = rating.saturating_sub(1);
    }
    if has_config(scan, &ConfigKind::Lockfile) {
        rating = rating.saturating_sub(1);
    }
    if has_config(scan, &ConfigKind::Docker) && q.dockerfile_has_useful_commands {
        rating = rating.saturating_sub(1);
    }
    if has_config(scan, &ConfigKind::BuildScript) && q.build_script_has_standard_targets {
        rating = rating.saturating_sub(1);
    }
    if has_config(scan, &ConfigKind::GitIgnore) {
        rating = rating.saturating_sub(1);
    }

    let mut evidence_parts: Vec<String> = Vec::new();
    if !has_config(scan, &ConfigKind::DependencyManager) {
        evidence_parts.push("no dependency manager".into());
    }
    if !has_config(scan, &ConfigKind::Lockfile) {
        evidence_parts.push("no lockfile".into());
    }
    if has_config(scan, &ConfigKind::Docker) {
        if q.dockerfile_has_useful_commands {
            evidence_parts.push("Docker with build/run commands".into());
        } else {
            evidence_parts.push("Docker present but incomplete".into());
        }
    } else {
        evidence_parts.push("no Docker config".into());
    }
    if has_config(scan, &ConfigKind::BuildScript) && !q.build_script_has_standard_targets {
        evidence_parts.push("build script missing standard targets".into());
    }

    let evidence = if evidence_parts.is_empty() {
        "All setup signals present and valid".to_string()
    } else {
        evidence_parts.join(", ")
    };

    DimensionScore {
        name: "Setup reliability".to_string(),
        weight: 10,
        rating,
        evidence,
    }
}

fn architecture_clarity(scan: &ScanResult, analysis: &AnalysisResult) -> DimensionScore {
    let mut rating = 0u32;

    if scan.max_dir_depth > 8 {
        rating += 2;
    } else if scan.max_dir_depth > 5 {
        rating += 1;
    }

    // Only count large files with meaningful logic, not data files (enums, dictionaries)
    let complex_large_files = scan
        .files
        .iter()
        .filter(|f| {
            if f.line_count <= 500 {
                return false;
            }
            let fn_count = analysis
                .functions
                .iter()
                .filter(|func| func.file == f.path)
                .count();
            fn_count > 0 && (fn_count as f64 / f.line_count as f64) > 0.002
        })
        .count();
    if complex_large_files > 3 {
        rating += 1;
    }

    if scan.total_files > 500 {
        rating += 1;
    }

    if analysis.layer_violations > 0 {
        rating += 1;
    }

    if analysis.god_module_count > 0 {
        rating += 1;
    }

    if analysis.duplicate_filename_count > 10 {
        rating += 1;
    }

    if analysis.function_collision_count > 15 {
        rating += 1;
    }

    rating = rating.min(5);

    let total_large = scan.files.iter().filter(|f| f.line_count > 500).count();
    let mut evidence = format!(
        "{} files, max depth {}, {} complex files >500 lines ({} total >500 lines, largest: {} lines)",
        scan.total_files, scan.max_dir_depth, complex_large_files, total_large, scan.max_file_lines
    );

    if analysis.layer_violations > 0 {
        evidence.push_str(&format!(
            ", {} bidirectional layer deps",
            analysis.layer_violations
        ));
    }
    if analysis.god_module_count > 0 {
        evidence.push_str(&format!(
            ", {} god modules (imported by >60% of groups)",
            analysis.god_module_count
        ));
    }
    if analysis.duplicate_filename_count > 0 {
        let dup_detail = match &analysis.worst_duplicate_filename {
            Some((name, count)) => format!(
                ", {} duplicate filenames (worst: {} x{})",
                analysis.duplicate_filename_count, name, count
            ),
            None => format!(
                ", {} duplicate filenames",
                analysis.duplicate_filename_count
            ),
        };
        evidence.push_str(&dup_detail);
    }
    if analysis.function_collision_count > 0 {
        let coll_detail = match &analysis.worst_function_collision {
            Some((name, count)) => format!(
                ", {} function name collisions (worst: {} in {} files)",
                analysis.function_collision_count, name, count
            ),
            None => format!(
                ", {} function name collisions",
                analysis.function_collision_count
            ),
        };
        evidence.push_str(&coll_detail);
    }

    DimensionScore {
        name: "Architecture clarity".to_string(),
        weight: 15,
        rating,
        evidence,
    }
}

fn coupling_blast_radius(analysis: &AnalysisResult) -> DimensionScore {
    let graph = imports::compute_import_graph(&analysis.imports);
    let mut rating = 0u32;

    if graph.avg_fan_out > 15.0 {
        rating += 2;
    } else if graph.avg_fan_out > 8.0 {
        rating += 1;
    }

    if graph.max_fan_out > 30 {
        rating += 1;
    }

    if graph.circular_dep_count > 5 {
        rating += 2;
    } else if graph.circular_dep_count > 0 {
        rating += 1;
    }

    rating = rating.min(5);

    let max_fan_out_label = graph
        .max_fan_out_file
        .as_ref()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default();

    let max_fan_in_label = graph.max_fan_in_module.as_deref().unwrap_or("");

    let cycle_detail = if graph.largest_cycle_size > 0 {
        format!(
            ", {} circular deps (largest: {} modules)",
            graph.circular_dep_count, graph.largest_cycle_size
        )
    } else {
        ", 0 circular deps".to_string()
    };

    let evidence = format!(
        "avg fan-out: {:.1}, max fan-out: {}{}, avg fan-in: {:.1}, max fan-in: {}{}{}",
        graph.avg_fan_out,
        graph.max_fan_out,
        if max_fan_out_label.is_empty() {
            String::new()
        } else {
            format!(" ({})", max_fan_out_label)
        },
        graph.avg_fan_in,
        graph.max_fan_in,
        if max_fan_in_label.is_empty() {
            String::new()
        } else {
            format!(" ({})", max_fan_in_label)
        },
        cycle_detail
    );

    DimensionScore {
        name: "Coupling / blast radius".to_string(),
        weight: 15,
        rating,
        evidence,
    }
}

fn style_consistency(scan: &ScanResult, analysis: &AnalysisResult) -> DimensionScore {
    let mut rating = 3u32; // default: unknown

    if has_config(scan, &ConfigKind::Formatter) {
        rating = rating.saturating_sub(2);
    }
    if has_config(scan, &ConfigKind::Linter) {
        rating = rating.saturating_sub(1);
    }

    let naming_ratio = analysis.naming.dominant_style_ratio();
    if naming_ratio < 0.6 {
        rating = (rating + 2).min(5);
    } else if naming_ratio < 0.8 {
        rating = (rating + 1).min(5);
    }

    let mut evidence_parts = Vec::new();
    if has_config(scan, &ConfigKind::Formatter) {
        evidence_parts.push("formatter configured");
    }
    if has_config(scan, &ConfigKind::Linter) {
        evidence_parts.push("linter configured");
    }
    evidence_parts.push(&format!("naming consistency: {:.0}%", naming_ratio * 100.0));

    // Can't borrow from a temporary, build evidence manually
    let evidence = format!(
        "{}naming consistency: {:.0}%",
        if has_config(scan, &ConfigKind::Formatter) && has_config(scan, &ConfigKind::Linter) {
            "formatter + linter configured, "
        } else if has_config(scan, &ConfigKind::Formatter) {
            "formatter configured, "
        } else if has_config(scan, &ConfigKind::Linter) {
            "linter configured, "
        } else {
            "no formatter/linter, "
        },
        naming_ratio * 100.0
    );

    DimensionScore {
        name: "Style consistency".to_string(),
        weight: 10,
        rating,
        evidence,
    }
}

fn test_safety_net(scan: &ScanResult) -> DimensionScore {
    let test_ratio = if scan.source_file_count > 0 {
        scan.test_file_count as f64 / scan.source_file_count as f64
    } else {
        0.0
    };

    let has_test_framework = has_config(scan, &ConfigKind::TestFramework);

    let mut rating = 5u32;

    if test_ratio > 0.5 {
        rating = rating.saturating_sub(2);
    } else if test_ratio > 0.2 {
        rating = rating.saturating_sub(1);
    }

    if has_test_framework {
        rating = rating.saturating_sub(1);
    }

    if scan.test_file_count > 0 {
        rating = rating.saturating_sub(1);
    }

    let evidence = format!(
        "test ratio: {:.2} ({} test files / {} source files){}",
        test_ratio,
        scan.test_file_count,
        scan.source_file_count,
        if has_test_framework {
            ", test framework configured"
        } else {
            ", no test framework config"
        }
    );

    DimensionScore {
        name: "Test safety net".to_string(),
        weight: 15,
        rating,
        evidence,
    }
}

fn runtime_predictability(analysis: &AnalysisResult) -> DimensionScore {
    let mut rating = 0u32;

    if analysis.global_mutable_count > 20 {
        rating += 2;
    } else if analysis.global_mutable_count > 0 {
        rating += 1;
    }

    let side_effect_patterns = analysis
        .anti_patterns
        .iter()
        .filter(|p| p.pattern_name == "debug_print" || p.pattern_name == "bare_except")
        .count();

    if side_effect_patterns > 5 {
        rating += 1;
    }

    if analysis.runtime_hazard_count > 10 {
        rating += 2;
    } else if analysis.runtime_hazard_count > 3 {
        rating += 1;
    }

    rating = rating.min(5);

    let hazard_detail = if analysis.runtime_hazard_count > 0 {
        format!(
            ", {} runtime hazards (singletons, module-level calls, event listeners)",
            analysis.runtime_hazard_count
        )
    } else {
        String::new()
    };

    let evidence = format!(
        "{} global mutable patterns, {} side-effect anti-patterns{}",
        analysis.global_mutable_count, side_effect_patterns, hazard_detail
    );

    DimensionScore {
        name: "Runtime predictability".to_string(),
        weight: 10,
        rating,
        evidence,
    }
}

fn feedback_loop_speed(scan: &ScanResult) -> DimensionScore {
    let mut rating = 0u32;
    let est = scan.quality.estimated_build_seconds;

    if est > 120.0 {
        rating += 3;
    } else if est > 30.0 {
        rating += 2;
    } else if est > 5.0 {
        rating += 1;
    }

    if !has_config(scan, &ConfigKind::TestFramework) {
        rating += 1;
    }

    if !has_config(scan, &ConfigKind::CI) && !has_config(scan, &ConfigKind::BuildScript) {
        rating += 1;
    }

    rating = rating.min(5);

    let build_est_label = if est > 60.0 {
        format!("~{:.0}min", est / 60.0)
    } else {
        format!("~{:.0}s", est)
    };

    let evidence = format!(
        "estimated build: {}, {}{}",
        build_est_label,
        if has_config(scan, &ConfigKind::TestFramework) {
            "test runner configured"
        } else {
            "no test runner"
        },
        if has_config(scan, &ConfigKind::CI) {
            ", CI configured"
        } else {
            ""
        }
    );

    DimensionScore {
        name: "Feedback loop speed".to_string(),
        weight: 5,
        rating,
        evidence,
    }
}

fn documentation(scan: &ScanResult) -> DimensionScore {
    let mut rating = 5u32;
    let q = &scan.quality;

    if has_config(scan, &ConfigKind::Readme) {
        let readme_ratio = if scan.total_bytes > 0 {
            q.readme_bytes as f64 / scan.total_bytes as f64
        } else {
            0.0
        };

        if q.readme_has_setup_instructions && readme_ratio > 0.005 {
            rating = rating.saturating_sub(2);
        } else if q.readme_bytes > 200 {
            rating = rating.saturating_sub(1);
        }
    }
    if has_config(scan, &ConfigKind::ArchitectureDocs) {
        rating = rating.saturating_sub(2);
    }
    if has_config(scan, &ConfigKind::Contributing) {
        rating = rating.saturating_sub(1);
    }

    let mut evidence_parts = Vec::new();
    if has_config(scan, &ConfigKind::Readme) {
        if q.readme_has_setup_instructions {
            evidence_parts.push(format!(
                "README with setup instructions ({} bytes)",
                q.readme_bytes
            ));
        } else if q.readme_bytes > 200 {
            evidence_parts.push(format!(
                "README present but no setup instructions ({} bytes)",
                q.readme_bytes
            ));
        } else {
            evidence_parts.push(format!("README too sparse ({} bytes)", q.readme_bytes));
        }
    } else {
        evidence_parts.push("no README".to_string());
    }
    if has_config(scan, &ConfigKind::ArchitectureDocs) {
        evidence_parts.push("architecture docs present".to_string());
    } else {
        evidence_parts.push("no architecture docs".to_string());
    }

    DimensionScore {
        name: "Documentation".to_string(),
        weight: 10,
        rating,
        evidence: evidence_parts.join(", "),
    }
}

fn dependency_boundaries(scan: &ScanResult) -> DimensionScore {
    let mut rating = 3u32;
    let q = &scan.quality;

    if has_config(scan, &ConfigKind::GitIgnore) {
        rating = rating.saturating_sub(1);
    }

    let has_vendor_or_generated = scan.files.iter().any(|f| {
        let path_str = f.path.to_string_lossy();
        path_str.contains("vendor/")
            || path_str.contains("generated/")
            || path_str.contains("gen/")
            || path_str.contains("proto/")
    });

    if has_vendor_or_generated {
        rating = rating.saturating_sub(1);
    }

    if has_config(scan, &ConfigKind::Lockfile) && q.lockfile_appears_fresh {
        rating = rating.saturating_sub(1);
    }

    let lockfile_status = if has_config(scan, &ConfigKind::Lockfile) {
        if q.lockfile_appears_fresh {
            ", lockfile present and fresh"
        } else {
            ", lockfile present but may be stale"
        }
    } else {
        ", no lockfile"
    };

    let evidence = format!(
        "{}{}{}",
        if has_config(scan, &ConfigKind::GitIgnore) {
            ".gitignore present"
        } else {
            "no .gitignore"
        },
        if has_vendor_or_generated {
            ", vendor/generated dirs separated"
        } else {
            ""
        },
        lockfile_status
    );

    DimensionScore {
        name: "Dependency boundaries".to_string(),
        weight: 5,
        rating,
        evidence,
    }
}

fn context_pressure(scan: &ScanResult, analysis: &AnalysisResult) -> DimensionScore {
    use super::context_budget;

    let mut rating = 0u32;

    let effective_bytes = match &scan.git_activity {
        Some(g) if g.is_git_repo && g.active_files > 0 => g.active_bytes,
        _ => scan.total_bytes,
    };
    let estimated_tokens = context_budget::estimate_total_tokens(scan);
    let active_token_ratio = effective_bytes as f64 / scan.total_bytes.max(1) as f64;
    let active_tokens = (estimated_tokens as f64 * active_token_ratio) as u64;

    if active_tokens > 500_000 {
        rating += 2;
    } else if active_tokens > 100_000 {
        rating += 1;
    }

    if analysis.max_function_lines > 300 {
        rating += 2;
    } else if analysis.max_function_lines > 100 {
        rating += 1;
    }

    if analysis.max_nesting_depth > 8 {
        rating += 1;
    }

    // Cross-reference: hot files that are also complex are the most dangerous
    let hot_complex_count = count_hot_complex_files(scan, analysis);
    if hot_complex_count > 3 {
        rating += 1;
    }

    rating = rating.min(5);

    let mut evidence = if effective_bytes < scan.total_bytes {
        format!(
            "~{} active tokens (~{} total), avg function {} lines, max function {} lines, max nesting depth {}",
            format_tokens(active_tokens as usize),
            format_tokens(estimated_tokens),
            analysis.avg_function_lines,
            analysis.max_function_lines,
            analysis.max_nesting_depth
        )
    } else {
        format!(
            "~{} estimated tokens, avg function {} lines, max function {} lines, max nesting depth {}",
            format_tokens(estimated_tokens),
            analysis.avg_function_lines,
            analysis.max_function_lines,
            analysis.max_nesting_depth
        )
    };

    if hot_complex_count > 0 {
        evidence.push_str(&format!(
            ", {} hot+complex files (frequently changed AND high complexity)",
            hot_complex_count
        ));
    }

    if analysis.unreferenced_function_count > 0 {
        evidence.push_str(&format!(
            ", ~{} potentially unused functions (~{} lines)",
            analysis.unreferenced_function_count, analysis.unreferenced_lines
        ));
    }

    DimensionScore {
        name: "Context pressure".to_string(),
        weight: 5,
        rating,
        evidence,
    }
}

/// Count files that are both frequently changed (hot) AND contain complex functions.
/// These are the highest-risk files for an agent — they change often AND are hard to understand.
fn count_hot_complex_files(scan: &ScanResult, analysis: &AnalysisResult) -> usize {
    let git = match &scan.git_activity {
        Some(g) if g.is_git_repo && !g.hot_files.is_empty() => g,
        _ => return 0,
    };

    let mut count = 0;
    for hot in &git.hot_files {
        let file_complexity: usize = analysis
            .functions
            .iter()
            .filter(|f| f.file == hot.path)
            .map(|f| f.cyclomatic_complexity)
            .sum();
        // A hot file with total complexity > 50 is dangerous
        if file_complexity > 50 {
            count += 1;
        }
    }
    count
}

fn format_tokens(tokens: usize) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        format!("{}", tokens)
    }
}
