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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dimensions(ratings: &[(u32, u32)]) -> Vec<DimensionScore> {
        ratings
            .iter()
            .enumerate()
            .map(|(i, &(weight, rating))| DimensionScore {
                name: format!("dim_{}", i),
                weight,
                rating,
                evidence: String::new(),
            })
            .collect()
    }

    #[test]
    fn verdict_clean() {
        assert_eq!(
            match 15u32 {
                0..=20 => "CLEAN",
                21..=40 => "ACCEPTABLE",
                41..=60 => "MESSY",
                61..=80 => "SLOPPY",
                _ => "DISASTER",
            },
            "CLEAN"
        );
    }

    #[test]
    fn verdict_acceptable() {
        assert_eq!(
            match 35u32 {
                0..=20 => "CLEAN",
                21..=40 => "ACCEPTABLE",
                41..=60 => "MESSY",
                61..=80 => "SLOPPY",
                _ => "DISASTER",
            },
            "ACCEPTABLE"
        );
    }

    #[test]
    fn verdict_messy() {
        assert_eq!(
            match 55u32 {
                0..=20 => "CLEAN",
                21..=40 => "ACCEPTABLE",
                41..=60 => "MESSY",
                61..=80 => "SLOPPY",
                _ => "DISASTER",
            },
            "MESSY"
        );
    }

    #[test]
    fn verdict_sloppy() {
        assert_eq!(
            match 75u32 {
                0..=20 => "CLEAN",
                21..=40 => "ACCEPTABLE",
                41..=60 => "MESSY",
                61..=80 => "SLOPPY",
                _ => "DISASTER",
            },
            "SLOPPY"
        );
    }

    #[test]
    fn verdict_disaster() {
        assert_eq!(
            match 95u32 {
                0..=20 => "CLEAN",
                21..=40 => "ACCEPTABLE",
                41..=60 => "MESSY",
                61..=80 => "SLOPPY",
                _ => "DISASTER",
            },
            "DISASTER"
        );
    }

    #[test]
    fn score_formula_all_zeros() {
        let dims = make_dimensions(&[(10, 0), (15, 0), (15, 0)]);
        let raw: f64 = dims
            .iter()
            .map(|d| d.weight as f64 * d.rating as f64 / 5.0)
            .sum();
        assert!((raw - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn score_formula_all_fives() {
        let dims = make_dimensions(&[
            (10, 5),
            (15, 5),
            (15, 5),
            (10, 5),
            (15, 5),
            (10, 5),
            (5, 5),
            (10, 5),
            (5, 5),
            (5, 5),
        ]);
        let raw: f64 = dims
            .iter()
            .map(|d| d.weight as f64 * d.rating as f64 / 5.0)
            .sum();
        assert!((raw - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn score_capped_at_100() {
        let raw = 150.0f64;
        let index = (raw.round() as u32).min(100);
        assert_eq!(index, 100);
    }

    #[test]
    fn score_formula_partial() {
        // weight=10 rating=3 -> 10*3/5 = 6
        let dims = make_dimensions(&[(10, 3)]);
        let raw: f64 = dims
            .iter()
            .map(|d| d.weight as f64 * d.rating as f64 / 5.0)
            .sum();
        assert!((raw - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn verdict_boundaries() {
        for i in 0..=20 {
            assert_eq!(
                match i {
                    0..=20 => "CLEAN",
                    21..=40 => "ACCEPTABLE",
                    41..=60 => "MESSY",
                    61..=80 => "SLOPPY",
                    _ => "DISASTER",
                },
                "CLEAN"
            );
        }
        assert_eq!(
            match 21u32 {
                0..=20 => "CLEAN",
                21..=40 => "ACCEPTABLE",
                _ => "OTHER",
            },
            "ACCEPTABLE"
        );
        assert_eq!(
            match 40u32 {
                0..=20 => "CLEAN",
                21..=40 => "ACCEPTABLE",
                _ => "OTHER",
            },
            "ACCEPTABLE"
        );
    }
}
