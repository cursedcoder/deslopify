use crate::analysis::imports;
use crate::analysis::AnalysisResult;
use crate::scanner::language::Language;
use crate::scanner::ScanResult;

use super::ContextBudget;

pub const USABLE_CONTEXT: usize = 176_000; // ~200k minus ~24k for system prompt

/// Average chars-per-token by language, approximating cl100k_base behavior.
/// Code has more symbols and short identifiers than prose, so ratios are lower.
fn chars_per_token(lang: Language) -> f64 {
    match lang {
        Language::Rust | Language::C | Language::Cpp | Language::Java => 3.2,
        Language::Go => 3.4,
        Language::Python | Language::Ruby => 3.5,
        Language::JavaScript | Language::Jsx => 3.8,
        Language::TypeScript | Language::Tsx => 3.6,
        _ => 3.5,
    }
}

/// Estimate token count for a piece of content using language-aware heuristics.
/// Counts words and symbol characters separately — each symbol is ~1 token,
/// each word averages 1-2 tokens depending on length.
pub fn estimate_tokens_for_content(content: &str, lang: Language) -> usize {
    if content.is_empty() {
        return 0;
    }

    let mut word_chars: usize = 0;
    let mut symbol_count: usize = 0;
    let mut whitespace_run = true;

    for ch in content.chars() {
        if ch.is_whitespace() {
            whitespace_run = true;
        } else if ch.is_alphanumeric() || ch == '_' {
            word_chars += 1;
            whitespace_run = false;
        } else {
            symbol_count += 1;
            if !whitespace_run {
                word_chars += 0; // end of word boundary handled by ratio
            }
            whitespace_run = false;
        }
    }

    let cpt = chars_per_token(lang);
    let word_tokens = (word_chars as f64 / cpt).ceil() as usize;
    word_tokens + symbol_count
}

/// Estimate total tokens for the entire scanned codebase using per-language ratios.
pub fn estimate_total_tokens(scan: &ScanResult) -> usize {
    let mut total = 0usize;
    for breakdown in &scan.language_breakdown {
        let cpt = chars_per_token(breakdown.language);
        total += (breakdown.total_bytes as f64 / cpt).ceil() as usize;
    }
    total
}

pub fn estimate(scan: &ScanResult, analysis: &AnalysisResult) -> ContextBudget {
    let graph = imports::compute_import_graph(&analysis.imports);

    let file_discovery_tokens = estimate_discovery_tokens(scan);
    let code_reading_tokens = estimate_reading_tokens(scan);
    let dependency_tracing_tokens = estimate_tracing_tokens(&graph);
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
    let search_queries = 3;
    let results_per_query = 20.min(scan.total_files);
    let tokens_per_result = 15;
    search_queries * results_per_query * tokens_per_result
}

fn estimate_reading_tokens(scan: &ScanResult) -> usize {
    let files_to_read = 7.min(scan.total_files);
    let dominant_lang = scan
        .language_breakdown
        .iter()
        .max_by_key(|b| b.file_count)
        .map(|b| b.language)
        .unwrap_or(Language::Unknown);
    let cpt = chars_per_token(dominant_lang);
    let avg_chars_per_line = 35.0; // typical code line
    let avg_file_tokens = (scan.avg_file_lines as f64 * avg_chars_per_line / cpt).ceil() as usize;
    files_to_read * avg_file_tokens
}

fn estimate_tracing_tokens(graph: &imports::ImportGraphStats) -> usize {
    let trace_depth = 2;
    let imports_per_file = graph.avg_fan_out as usize;
    let tokens_per_trace = 50;
    trace_depth * imports_per_file * tokens_per_trace
}

fn estimate_comprehension_tokens(analysis: &AnalysisResult) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_zero_tokens() {
        assert_eq!(estimate_tokens_for_content("", Language::Rust), 0);
    }

    #[test]
    fn rust_code_token_estimate() {
        let code = "fn main() {\n    let x = 42;\n    println!(\"{}\", x);\n}";
        let tokens = estimate_tokens_for_content(code, Language::Rust);
        assert!(
            tokens > 10 && tokens < 30,
            "Expected 10-30 tokens for small Rust snippet, got {}",
            tokens
        );
    }

    #[test]
    fn python_code_token_estimate() {
        let code = "def hello(name):\n    print(f\"Hello {name}\")\n    return True\n";
        let tokens = estimate_tokens_for_content(code, Language::Python);
        assert!(
            tokens > 8 && tokens < 25,
            "Expected 8-25 tokens for small Python snippet, got {}",
            tokens
        );
    }

    #[test]
    fn symbols_count_as_tokens() {
        let code = "{}()[];,.::<>=+-*/&|!~^%@#$";
        let tokens = estimate_tokens_for_content(code, Language::Rust);
        assert!(
            tokens >= 20,
            "Each symbol should be ~1 token, got {}",
            tokens
        );
    }

    #[test]
    fn language_multiplier_varies() {
        let code = "function processUserData(userData, callback) { return callback(userData); }";
        let js_tokens = estimate_tokens_for_content(code, Language::JavaScript);
        let rust_tokens = estimate_tokens_for_content(code, Language::Rust);
        assert!(
            rust_tokens > js_tokens,
            "Rust (lower cpt) should produce more tokens than JS: {} vs {}",
            rust_tokens,
            js_tokens
        );
    }
}
