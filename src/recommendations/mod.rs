pub mod engine;
pub mod templates;

use serde::Serialize;

use crate::analysis::AnalysisResult;
use crate::scanner::ScanResult;
use crate::scoring::ScoreResult;

#[derive(Debug, Serialize)]
pub struct Recommendation {
    pub priority: usize,
    pub estimated_reduction: u32,
    pub title: String,
    pub target: String,
    pub action: String,
    pub evidence: String,
}

pub fn generate(
    score: &ScoreResult,
    scan: &ScanResult,
    analysis: &AnalysisResult,
) -> Vec<Recommendation> {
    let mut recs = engine::generate_all(score, scan, analysis);
    recs.sort_by(|a, b| b.estimated_reduction.cmp(&a.estimated_reduction));
    for (i, rec) in recs.iter_mut().enumerate() {
        rec.priority = i + 1;
    }
    recs.truncate(10);
    recs
}
