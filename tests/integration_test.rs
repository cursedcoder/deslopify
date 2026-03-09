use std::path::PathBuf;

use deslopify::{analysis, recommendations, scanner, scoring};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn clean_project_scores_low() {
    let path = fixture_path("clean_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);
    let score = scoring::score(&scan, &analysis);

    assert!(
        score.slop_index <= 50,
        "Clean project should score <= 50, got {}",
        score.slop_index
    );
    assert!(
        score.verdict == "CLEAN" || score.verdict == "ACCEPTABLE" || score.verdict == "MESSY",
        "Clean project verdict should not be SLOPPY/DISASTER, got {}",
        score.verdict
    );
}

#[test]
fn sloppy_project_scores_higher_than_clean() {
    let clean_path = fixture_path("clean_project");
    let sloppy_path = fixture_path("sloppy_project");

    let clean_scan = scanner::scan(&[clean_path], &[]);
    let clean_analysis = analysis::analyze(&clean_scan);
    let clean_score = scoring::score(&clean_scan, &clean_analysis);

    let sloppy_scan = scanner::scan(&[sloppy_path], &[]);
    let sloppy_analysis = analysis::analyze(&sloppy_scan);
    let sloppy_score = scoring::score(&sloppy_scan, &sloppy_analysis);

    assert!(
        sloppy_score.slop_index >= clean_score.slop_index,
        "Sloppy project ({}) should score >= clean project ({})",
        sloppy_score.slop_index,
        clean_score.slop_index
    );
}

#[test]
fn clean_project_detects_configs() {
    let path = fixture_path("clean_project");
    let scan = scanner::scan(&[path], &[]);

    let has_readme = scan
        .configs
        .iter()
        .any(|c| c.kind == scanner::ConfigKind::Readme);
    let has_linter = scan
        .configs
        .iter()
        .any(|c| c.kind == scanner::ConfigKind::Linter);
    let has_test_framework = scan
        .configs
        .iter()
        .any(|c| c.kind == scanner::ConfigKind::TestFramework);
    let has_type_checker = scan
        .configs
        .iter()
        .any(|c| c.kind == scanner::ConfigKind::TypeChecker);
    let has_formatter = scan
        .configs
        .iter()
        .any(|c| c.kind == scanner::ConfigKind::Formatter);

    assert!(has_readme, "Should detect README.md");
    assert!(has_linter, "Should detect ruff in pyproject.toml");
    assert!(has_test_framework, "Should detect pytest in pyproject.toml");
    assert!(has_type_checker, "Should detect mypy in pyproject.toml");
    assert!(has_formatter, "Should detect black in pyproject.toml");
}

#[test]
fn clean_project_detects_test_files() {
    let path = fixture_path("clean_project");
    let scan = scanner::scan(&[path], &[]);

    assert!(
        scan.test_file_count > 0,
        "Should detect test files in tests/"
    );
}

#[test]
fn sloppy_project_detects_anti_patterns() {
    let path = fixture_path("sloppy_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);

    let todo_count = analysis
        .anti_patterns
        .iter()
        .filter(|p| p.pattern_name == "todo_placeholder")
        .count();
    let debug_count = analysis
        .anti_patterns
        .iter()
        .filter(|p| p.pattern_name == "debug_print")
        .count();
    let bare_except_count = analysis
        .anti_patterns
        .iter()
        .filter(|p| p.pattern_name == "bare_except")
        .count();
    let star_import_count = analysis
        .anti_patterns
        .iter()
        .filter(|p| p.pattern_name == "star_import")
        .count();

    assert!(todo_count >= 2, "Should detect TODOs, found {}", todo_count);
    assert!(
        debug_count >= 2,
        "Should detect print statements, found {}",
        debug_count
    );
    assert!(
        bare_except_count >= 1,
        "Should detect bare except, found {}",
        bare_except_count
    );
    assert!(
        star_import_count >= 1,
        "Should detect star import, found {}",
        star_import_count
    );
}

#[test]
fn sloppy_project_detects_functions() {
    let path = fixture_path("sloppy_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);

    assert!(
        !analysis.functions.is_empty(),
        "Should extract functions from Python files"
    );

    let fn_names: Vec<&str> = analysis.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        fn_names.contains(&"processData"),
        "Should find processData function"
    );
    assert!(
        fn_names.contains(&"calculate_total"),
        "Should find calculate_total function"
    );
}

#[test]
fn sloppy_project_detects_mixed_naming() {
    let path = fixture_path("sloppy_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);

    assert!(
        analysis.naming.snake_case_count > 0,
        "Should find snake_case names"
    );
    assert!(
        analysis.naming.camel_case_count > 0,
        "Should find camelCase names"
    );
    assert!(
        analysis.naming.dominant_style_ratio() < 1.0,
        "Mixed naming should have ratio < 1.0"
    );
}

#[test]
fn recommendations_generated_for_sloppy() {
    let path = fixture_path("sloppy_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);
    let score = scoring::score(&scan, &analysis);
    let recs = recommendations::generate(&score, &scan, &analysis);

    assert!(
        !recs.is_empty(),
        "Should generate recommendations for sloppy project"
    );
    assert!(recs[0].priority == 1, "First rec should have priority 1");
    assert!(
        recs[0].estimated_reduction > 0,
        "Recommendations should have positive reduction"
    );
}

#[test]
fn scan_respects_ignore_patterns() {
    let path = fixture_path("sloppy_project");
    let scan_all = scanner::scan(std::slice::from_ref(&path), &[]);
    let scan_ignored = scanner::scan(&[path], &["engine".to_string()]);

    assert!(
        scan_ignored.total_files < scan_all.total_files,
        "Ignoring 'engine' should reduce file count: {} vs {}",
        scan_ignored.total_files,
        scan_all.total_files
    );
}

#[test]
fn json_output_is_valid() {
    let path = fixture_path("clean_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);
    let score = scoring::score(&scan, &analysis);
    let recs = recommendations::generate(&score, &scan, &analysis);

    let json_str = serde_json::to_string(&serde_json::json!({
        "slop_index": score.slop_index,
        "verdict": score.verdict,
        "raw_score": score.raw_score,
        "dimensions": score.dimensions.len(),
        "recommendations": recs.len(),
    }))
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(parsed["slop_index"].is_number());
    assert!(parsed["verdict"].is_string());
}

#[test]
fn context_budget_is_reasonable() {
    let path = fixture_path("clean_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);
    let score = scoring::score(&scan, &analysis);

    let budget = &score.context_budget;
    assert_eq!(budget.usable_context, 176_000);
    assert!(budget.total_navigation_tokens > 0);
    assert!(budget.total_navigation_tokens < budget.usable_context);
    assert!(budget.remaining_tokens > 0);
    assert!((budget.navigation_pct) >= 0.0 && budget.navigation_pct < 100.0);
}

#[test]
fn all_ten_dimensions_present() {
    let path = fixture_path("clean_project");
    let scan = scanner::scan(&[path], &[]);
    let analysis = analysis::analyze(&scan);
    let score = scoring::score(&scan, &analysis);

    assert_eq!(
        score.dimensions.len(),
        10,
        "Should have exactly 10 dimensions"
    );

    let total_weight: u32 = score.dimensions.iter().map(|d| d.weight).sum();
    assert_eq!(total_weight, 100, "Weights should sum to 100");

    for dim in &score.dimensions {
        assert!(
            dim.rating <= 5,
            "{} rating {} exceeds 5",
            dim.name,
            dim.rating
        );
    }
}
