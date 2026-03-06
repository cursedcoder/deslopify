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
    dimensions: &'a [crate::scoring::DimensionScore],
    context_budget: &'a crate::scoring::ContextBudget,
    recommendations: &'a [Recommendation],
    summary: SummaryJson,
    duplicates: DuplicatesJson,
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
