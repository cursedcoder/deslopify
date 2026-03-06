use crate::analysis::imports;
use crate::analysis::AnalysisResult;
use crate::scanner::ScanResult;

use super::ContextBudget;

pub const USABLE_CONTEXT: usize = 176_000; // ~200k minus ~24k for system prompt
pub const CHARS_PER_TOKEN: usize = 4;

pub fn estimate(scan: &ScanResult, analysis: &AnalysisResult) -> ContextBudget {
    let graph = imports::compute_import_graph(&analysis.imports);

    // File discovery: proportional to total files and directory depth
    let file_discovery_tokens = estimate_discovery_tokens(scan);

    // Code reading: based on average file size and typical number of files an agent reads
    let code_reading_tokens = estimate_reading_tokens(scan, analysis);

    // Dependency tracing: based on fan-out and coupling
    let dependency_tracing_tokens = estimate_tracing_tokens(&graph, analysis);

    // Comprehension overhead: extra cost for complex/unclear code
    let comprehension_tokens = estimate_comprehension_tokens(analysis);

    let total_navigation_tokens = file_discovery_tokens
        + code_reading_tokens
        + dependency_tracing_tokens
        + comprehension_tokens;

    let navigation_pct = if USABLE_CONTEXT > 0 {
        total_navigation_tokens as f64 / USABLE_CONTEXT as f64 * 100.0
    } else {
        0.0
    };

    let remaining_tokens = USABLE_CONTEXT.saturating_sub(total_navigation_tokens);

    ContextBudget {
        file_discovery_tokens,
        code_reading_tokens,
        dependency_tracing_tokens,
        comprehension_tokens,
        total_navigation_tokens,
        usable_context: USABLE_CONTEXT,
        navigation_pct,
        remaining_tokens,
    }
}

fn estimate_discovery_tokens(scan: &ScanResult) -> usize {
    // Directory listing: ~10 tokens per file/dir entry for search results
    // Typical agent does 2-5 search queries
    let search_queries = 3;
    let results_per_query = 20.min(scan.total_files);
    let tokens_per_result = 15; // path + snippet
    search_queries * results_per_query * tokens_per_result
}

fn estimate_reading_tokens(scan: &ScanResult, _analysis: &AnalysisResult) -> usize {
    // Agent typically reads 5-10 files for a change
    let files_to_read = 7.min(scan.total_files);
    let avg_file_tokens = scan.avg_file_lines * 10 / CHARS_PER_TOKEN; // ~10 chars per line
    files_to_read * avg_file_tokens
}

fn estimate_tracing_tokens(graph: &imports::ImportGraphStats, _analysis: &AnalysisResult) -> usize {
    // Each import trace requires reading part of the target file
    // Cost proportional to average fan-out * depth
    let trace_depth = 2; // typical: 2 levels of dependency tracing
    let imports_per_file = graph.avg_fan_out as usize;
    let tokens_per_trace = 50; // reading import header + key declarations
    trace_depth * imports_per_file * tokens_per_trace
}

fn estimate_comprehension_tokens(analysis: &AnalysisResult) -> usize {
    // Extra cost for complex code that requires re-reading or explanation
    let complexity_overhead = if analysis.avg_cyclomatic_complexity > 10.0 {
        2000
    } else if analysis.avg_cyclomatic_complexity > 5.0 {
        1000
    } else {
        500
    };

    let nesting_overhead = if analysis.max_nesting_depth > 6 {
        1500
    } else if analysis.max_nesting_depth > 4 {
        800
    } else {
        300
    };

    let duplication_overhead = analysis.duplicates.len() * 100;

    complexity_overhead + nesting_overhead + duplication_overhead
}
