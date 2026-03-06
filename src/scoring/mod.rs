pub mod context_budget;
pub mod dimensions;
pub mod model;

use crate::analysis::AnalysisResult;
use crate::scanner::ScanResult;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DimensionScore {
    pub name: String,
    pub weight: u32,
    pub rating: u32, // 0-5
    pub evidence: String,
}

#[derive(Debug, Serialize)]
pub struct ContextBudget {
    pub file_discovery_tokens: usize,
    pub code_reading_tokens: usize,
    pub dependency_tracing_tokens: usize,
    pub comprehension_tokens: usize,
    pub total_navigation_tokens: usize,
    pub usable_context: usize,
    pub navigation_pct: f64,
    pub remaining_tokens: usize,
}

#[derive(Debug, Serialize)]
pub struct ScoreResult {
    pub slop_index: u32,
    pub raw_score: f64,
    pub verdict: String,
    pub dimensions: Vec<DimensionScore>,
    pub context_budget: ContextBudget,
}

pub fn score(scan: &ScanResult, analysis: &AnalysisResult) -> ScoreResult {
    let dimensions = dimensions::compute_all(scan, analysis);
    let raw_score: f64 = dimensions
        .iter()
        .map(|d| d.weight as f64 * d.rating as f64 / 5.0)
        .sum();

    let slop_index = (raw_score.round() as u32).min(100);

    let verdict = match slop_index {
        0..=20 => "CLEAN",
        21..=40 => "ACCEPTABLE",
        41..=60 => "MESSY",
        61..=80 => "SLOPPY",
        _ => "DISASTER",
    }
    .to_string();

    let context_budget = context_budget::estimate(scan, analysis);

    ScoreResult {
        slop_index,
        raw_score,
        verdict,
        dimensions,
        context_budget,
    }
}
