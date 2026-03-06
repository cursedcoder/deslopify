use clap::Parser;
use std::path::PathBuf;
use std::process;

mod analysis;
mod output;
mod recommendations;
mod scanner;
mod scoring;

#[derive(Parser)]
#[command(
    name = "deslopify",
    version,
    about = "Codebase LLM-Friction Analyzer — measure how hard it is for an LLM agent to work on your codebase"
)]
pub struct Cli {
    /// Paths to scan (defaults to current directory)
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Output format: terminal, json, llm
    #[arg(long, default_value = "terminal")]
    format: String,

    /// CI mode — exit 1 if any issues found
    #[arg(long)]
    ci: bool,

    /// Maximum allowed slop score (exit 1 if exceeded, implies --ci)
    #[arg(long)]
    max_score: Option<u32>,

    /// Glob patterns to ignore
    #[arg(long)]
    ignore: Vec<String>,

    /// Show all dimension breakdowns
    #[arg(long, short)]
    verbose: bool,

    /// Show estimated context-window token costs
    #[arg(long)]
    context_budget: bool,
}

fn main() {
    let cli = Cli::parse();

    let scan_result = scanner::scan(&cli.paths, &cli.ignore);
    let analysis_result = analysis::analyze(&scan_result);
    let score_result = scoring::score(&scan_result, &analysis_result);
    let recs = recommendations::generate(&score_result, &scan_result, &analysis_result);

    match cli.format.as_str() {
        "json" => output::json::print(&score_result, &recs, &scan_result, &analysis_result),
        "llm" => output::llm::print(&score_result, &recs, &scan_result),
        _ => output::terminal::print(
            &score_result,
            &recs,
            &scan_result,
            cli.verbose,
            cli.context_budget,
        ),
    }

    let exit_code = if let Some(max) = cli.max_score {
        if score_result.slop_index > max {
            1
        } else {
            0
        }
    } else if cli.ci && score_result.slop_index > 0 {
        1
    } else {
        0
    };

    process::exit(exit_code as i32);
}
