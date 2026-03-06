use owo_colors::OwoColorize;

use crate::recommendations::Recommendation;
use crate::scanner::ScanResult;
use crate::scoring::ScoreResult;

pub fn print(
    score: &ScoreResult,
    recs: &[Recommendation],
    scan: &ScanResult,
    verbose: bool,
    show_budget: bool,
) {
    println!();
    print_header(score);
    println!();
    print_summary(scan);
    println!();
    print_dimensions(score, verbose);

    if show_budget {
        println!();
        print_context_budget(score);
    }

    if !recs.is_empty() {
        println!();
        print_recommendations(recs, score);
    }

    println!();
}

fn print_header(score: &ScoreResult) {
    let verdict_colored = match score.verdict.as_str() {
        "CLEAN" => score.verdict.green().bold().to_string(),
        "ACCEPTABLE" => score.verdict.blue().bold().to_string(),
        "MESSY" => score.verdict.yellow().bold().to_string(),
        "SLOPPY" => score.verdict.red().bold().to_string(),
        "DISASTER" => score.verdict.on_red().white().bold().to_string(),
        _ => score.verdict.clone(),
    };

    let index_str = format!("{}/100", score.slop_index);
    let index_colored = match score.slop_index {
        0..=20 => index_str.green().bold().to_string(),
        21..=40 => index_str.blue().bold().to_string(),
        41..=60 => index_str.yellow().bold().to_string(),
        61..=80 => index_str.red().bold().to_string(),
        _ => index_str.on_red().white().bold().to_string(),
    };

    if score.size_multiplier < 0.95 {
        println!(
            "  {} {} ({})  {}",
            "SLOP INDEX:".bold(),
            index_colored,
            verdict_colored,
            format!(
                "size multiplier: {:.2}x — small codebase",
                score.size_multiplier
            )
            .dimmed()
        );
    } else {
        println!(
            "  {} {} ({})",
            "SLOP INDEX:".bold(),
            index_colored,
            verdict_colored
        );
    }
}

fn print_summary(scan: &ScanResult) {
    println!("  {}", "CODEBASE SUMMARY".bold().dimmed());
    println!(
        "    Files: {}  Lines: {}  Languages: {}",
        scan.total_files,
        format_number(scan.total_lines),
        scan.language_breakdown.len()
    );

    let langs: Vec<String> = scan
        .language_breakdown
        .iter()
        .take(5)
        .map(|b| format!("{} ({})", b.language.label(), b.file_count))
        .collect();
    println!("    {}", langs.join(", "));

    if let Some(ref git) = scan.git_activity {
        if git.is_git_repo && git.active_files > 0 {
            let pct = if scan.total_files > 0 {
                git.active_files as f64 / scan.total_files as f64 * 100.0
            } else {
                0.0
            };
            println!(
                "    Active surface: {} files ({:.0}%), {} lines changed in last {} months",
                git.active_files,
                pct,
                format_number(git.active_lines),
                git.window_months
            );

            let top: Vec<String> = git
                .hot_files
                .iter()
                .take(3)
                .map(|h| {
                    let name = h.path.file_name().unwrap_or_default().to_string_lossy();
                    format!("{} ({} commits)", name, h.commit_count)
                })
                .collect();
            if !top.is_empty() {
                println!("    Hot files: {}", top.join(", "));
            }
        }
    }

    let configs: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        scan.configs
            .iter()
            .filter(|c| seen.insert(c.kind.label()))
            .map(|c| c.kind.label().to_string())
            .collect()
    };
    if !configs.is_empty() {
        println!(
            "    Tooling: {}",
            configs[..configs.len().min(6)].join(", ")
        );
    }
}

fn print_dimensions(score: &ScoreResult, verbose: bool) {
    println!("  {}", "DIMENSION BREAKDOWN".bold().dimmed());

    let mut dims: Vec<&crate::scoring::DimensionScore> = score.dimensions.iter().collect();
    dims.sort_by(|a, b| {
        let score_a = a.weight as f64 * a.rating as f64 / 5.0;
        let score_b = b.weight as f64 * b.rating as f64 / 5.0;
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let show_count = if verbose {
        dims.len()
    } else {
        dims.len().min(6)
    };

    for dim in dims.iter().take(show_count) {
        let bar = render_bar(dim.rating, 5);
        let contribution = dim.weight as f64 * dim.rating as f64 / 5.0;

        let bar_colored = match dim.rating {
            0..=1 => bar.green().to_string(),
            2..=3 => bar.yellow().to_string(),
            _ => bar.red().to_string(),
        };

        println!(
            "    {:<25} {}  {}/5  ({:>2})  {}",
            dim.name,
            bar_colored,
            dim.rating,
            contribution as u32,
            dim.evidence.dimmed()
        );
    }

    if !verbose && dims.len() > 6 {
        println!(
            "    {}",
            format!("... {} more (use --verbose to show all)", dims.len() - 6).dimmed()
        );
    }
}

fn print_context_budget(score: &ScoreResult) {
    let budget = &score.context_budget;
    println!("  {}", "CONTEXT BUDGET ESTIMATE".bold().dimmed());
    println!(
        "    File discovery:     ~{:>6} tokens",
        format_number(budget.file_discovery_tokens)
    );
    println!(
        "    Code reading:       ~{:>6} tokens",
        format_number(budget.code_reading_tokens)
    );
    println!(
        "    Dependency tracing: ~{:>6} tokens",
        format_number(budget.dependency_tracing_tokens)
    );
    println!(
        "    Comprehension:      ~{:>6} tokens",
        format_number(budget.comprehension_tokens)
    );
    println!("    {}", "─".repeat(42));
    println!(
        "    Navigation cost:    ~{:>6} tokens  ({:.1}% of usable context)",
        format_number(budget.total_navigation_tokens),
        budget.navigation_pct
    );
    println!(
        "    Remaining for work: ~{:>6} tokens",
        format_number(budget.remaining_tokens)
    );
}

fn print_recommendations(recs: &[Recommendation], score: &ScoreResult) {
    println!(
        "  {}",
        "TOP FIXES (by estimated index reduction)".bold().dimmed()
    );

    let show_count = recs.len().min(5);
    let mut running_index = score.slop_index;

    for rec in recs.iter().take(show_count) {
        running_index = running_index.saturating_sub(rec.estimated_reduction);
        let reduction = format!("-{} pts", rec.estimated_reduction);
        println!(
            "    {}. [{}] {}",
            rec.priority,
            reduction.green(),
            rec.title
        );
    }

    if running_index < score.slop_index {
        println!();
        println!(
            "    Estimated index after all fixes: {} -> {}",
            score.slop_index,
            running_index.to_string().green()
        );
    }
}

fn render_bar(value: u32, max: u32) -> String {
    let filled = value as usize;
    let empty = (max as usize).saturating_sub(filled);
    format!("[{}{}]", "#".repeat(filled), "-".repeat(empty))
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}
