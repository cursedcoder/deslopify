pub mod context_budget;
pub mod dimensions;

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
    pub size_multiplier: f64,
    pub verdict: String,
    pub dimensions: Vec<DimensionScore>,
    pub context_budget: ContextBudget,
}

/// Compute a multiplier [0.0, 1.0] based on the effective codebase size relative
/// to the usable context window. When git data is available, uses active surface
/// bytes (files changed in the lookback window) instead of total bytes — frozen
/// code is like a library the agent reads but doesn't modify.
pub fn size_multiplier(total_bytes: u64, git: Option<&crate::scanner::git::GitActivity>) -> f64 {
    let effective_bytes = match git {
        Some(g) if g.is_git_repo && g.active_files > 0 => g.active_bytes,
        _ => total_bytes,
    };
    let estimated_tokens = effective_bytes as f64 / context_budget::CHARS_PER_TOKEN as f64;
    let context_ratio = estimated_tokens / context_budget::USABLE_CONTEXT as f64;
    context_ratio.powf(1.5).min(1.0)
}

pub fn score(scan: &ScanResult, analysis: &AnalysisResult) -> ScoreResult {
    let dimensions = dimensions::compute_all(scan, analysis);
    let raw_score: f64 = dimensions
        .iter()
        .map(|d| d.weight as f64 * d.rating as f64 / 5.0)
        .sum();

    let mult = size_multiplier(scan.total_bytes, scan.git_activity.as_ref());
    let adjusted = raw_score * mult;
    let slop_index = (adjusted.round() as u32).min(100);

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
        size_multiplier: mult,
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
    fn size_multiplier_tiny_project() {
        let mult = size_multiplier(10_000, None);
        assert!(
            mult < 0.01,
            "Tiny project should have multiplier < 0.01, got {:.4}",
            mult
        );
        assert!(mult > 0.0, "Multiplier should be positive");
    }

    #[test]
    fn size_multiplier_small_project() {
        let mult = size_multiplier(70_000, None);
        assert!(
            mult < 0.05,
            "Small project should have multiplier < 0.05, got {:.4}",
            mult
        );
    }

    #[test]
    fn size_multiplier_quarter_context() {
        let mult = size_multiplier(176_000, None);
        assert!(
            mult < 0.15,
            "Quarter-context project should have multiplier < 0.15, got {:.3}",
            mult
        );
    }

    #[test]
    fn size_multiplier_half_context() {
        let mult = size_multiplier(352_000, None);
        assert!(
            mult > 0.3 && mult < 0.4,
            "Half-context project should have multiplier ~0.35, got {:.3}",
            mult
        );
    }

    #[test]
    fn size_multiplier_full_context() {
        let mult = size_multiplier(704_000, None);
        assert!(
            mult > 0.95,
            "Full-context project should have multiplier ~1.0, got {:.3}",
            mult
        );
    }

    #[test]
    fn size_multiplier_large_project() {
        let mult = size_multiplier(4_000_000, None);
        assert!(
            (mult - 1.0).abs() < f64::EPSILON,
            "Large project should have multiplier exactly 1.0, got {:.3}",
            mult
        );
    }

    #[test]
    fn size_multiplier_zero_bytes() {
        let mult = size_multiplier(0, None);
        assert!(
            (mult - 0.0).abs() < f64::EPSILON,
            "Empty project should have multiplier 0.0"
        );
    }

    #[test]
    fn size_multiplier_uses_active_bytes_from_git() {
        use crate::scanner::git::GitActivity;
        let git = GitActivity {
            is_git_repo: true,
            active_files: 10,
            active_lines: 1000,
            active_bytes: 30_000,
            frozen_files: 50,
            frozen_bytes: 670_000,
            hot_files: Vec::new(),
            total_commits: 100,
            window_months: 6,
        };
        let with_git = size_multiplier(700_000, Some(&git));
        let without_git = size_multiplier(700_000, None);
        assert!(
            with_git < without_git * 0.1,
            "Git active surface (30k) should produce much smaller multiplier than total (700k): {:.4} vs {:.4}",
            with_git, without_git
        );
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
