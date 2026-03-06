use crate::recommendations::Recommendation;
use crate::scanner::ScanResult;
use crate::scoring::ScoreResult;

pub fn print(score: &ScoreResult, recs: &[Recommendation], scan: &ScanResult) {
    println!("CODEBASE IMPROVEMENT INSTRUCTIONS");
    println!(
        "Current Slop Index: {}/100 ({})",
        score.slop_index, score.verdict
    );
    println!();
    println!(
        "Codebase: {} files, {} lines across {} languages.",
        scan.total_files,
        scan.total_lines,
        scan.language_breakdown.len()
    );
    println!();
    println!(
        "Context budget: ~{} tokens navigation cost ({:.1}% of usable context).",
        score.context_budget.total_navigation_tokens, score.context_budget.navigation_pct
    );
    println!();
    println!("You are tasked with reducing the Slop Index of this codebase.");
    println!(
        "Execute these fixes in priority order. After each fix, the estimated new index is shown."
    );
    println!();

    let mut running_index = score.slop_index;

    for rec in recs {
        running_index = running_index.saturating_sub(rec.estimated_reduction);
        println!(
            "PRIORITY {}: {} (estimated index after: {})",
            rec.priority, rec.title, running_index
        );
        println!("Target: {}", rec.target);
        println!("Evidence: {}", rec.evidence);
        println!("Action: {}", rec.action);
        println!();
    }

    println!("DIMENSION DETAILS");
    println!();
    for dim in &score.dimensions {
        println!(
            "- {} (score {}/5, weight {}): {}",
            dim.name, dim.rating, dim.weight, dim.evidence
        );
    }
}
