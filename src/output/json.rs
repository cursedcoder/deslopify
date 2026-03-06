use serde::Serialize;

use crate::analysis::AnalysisResult;
use crate::recommendations::Recommendation;
use crate::scanner::ScanResult;
use crate::scoring::ScoreResult;

#[derive(Serialize)]
struct JsonReport<'a> {
    slop_index: u32,
    verdict: &'a str,
    raw_score: f64,
    size_multiplier: f64,
    dimensions: &'a [crate::scoring::DimensionScore],
    context_budget: &'a crate::scoring::ContextBudget,
    recommendations: &'a [Recommendation],
    summary: SummaryJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_surface: Option<ActiveSurfaceJson>,
    duplicates: DuplicatesJson,
}

#[derive(Serialize)]
struct ActiveSurfaceJson {
    active_files: usize,
    active_lines: usize,
    active_bytes: u64,
    frozen_files: usize,
    frozen_bytes: u64,
    window_months: u32,
    total_commits: usize,
    hot_files: Vec<HotFileJson>,
}

#[derive(Serialize)]
struct HotFileJson {
    path: String,
    commit_count: usize,
    lines: usize,
}

#[derive(Serialize)]
struct DuplicatesJson {
    cluster_count: usize,
    total_duplicate_lines: usize,
}

#[derive(Serialize)]
struct SummaryJson {
    total_files: usize,
    total_lines: usize,
    total_bytes: u64,
    max_file_lines: usize,
    avg_file_lines: usize,
    languages: Vec<LanguageJson>,
    test_files: usize,
    source_files: usize,
    configs: Vec<String>,
}

#[derive(Serialize)]
struct LanguageJson {
    name: String,
    files: usize,
    lines: usize,
    bytes: u64,
}

pub fn print(
    score: &ScoreResult,
    recs: &[Recommendation],
    scan: &ScanResult,
    analysis: &AnalysisResult,
) {
    let report = JsonReport {
        slop_index: score.slop_index,
        verdict: &score.verdict,
        raw_score: score.raw_score,
        size_multiplier: score.size_multiplier,
        dimensions: &score.dimensions,
        context_budget: &score.context_budget,
        recommendations: recs,
        summary: SummaryJson {
            total_files: scan.total_files,
            total_lines: scan.total_lines,
            total_bytes: scan.total_bytes,
            max_file_lines: scan.max_file_lines,
            avg_file_lines: scan.avg_file_lines,
            languages: scan
                .language_breakdown
                .iter()
                .map(|b| LanguageJson {
                    name: b.language.label().to_string(),
                    files: b.file_count,
                    lines: b.total_lines,
                    bytes: b.total_bytes,
                })
                .collect(),
            test_files: scan.test_file_count,
            source_files: scan.source_file_count,
            configs: scan
                .configs
                .iter()
                .map(|c| format!("{}: {}", c.kind.label(), c.path.display()))
                .collect(),
        },
        active_surface: scan.git_activity.as_ref().and_then(|g| {
            if !g.is_git_repo || g.active_files == 0 {
                return None;
            }
            Some(ActiveSurfaceJson {
                active_files: g.active_files,
                active_lines: g.active_lines,
                active_bytes: g.active_bytes,
                frozen_files: g.frozen_files,
                frozen_bytes: g.frozen_bytes,
                window_months: g.window_months,
                total_commits: g.total_commits,
                hot_files: g
                    .hot_files
                    .iter()
                    .map(|h| HotFileJson {
                        path: h.path.display().to_string(),
                        commit_count: h.commit_count,
                        lines: h.lines,
                    })
                    .collect(),
            })
        }),
        duplicates: DuplicatesJson {
            cluster_count: analysis.duplicates.len(),
            total_duplicate_lines: analysis
                .duplicates
                .iter()
                .map(|d| d.line_count * d.locations.len())
                .sum(),
        },
    };

    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializing JSON: {}", e),
    }
}
