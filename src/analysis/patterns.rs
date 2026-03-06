use regex::Regex;
use std::path::Path;

use crate::scanner::FileEntry;

use super::PatternMatch;

struct AntiPattern {
    name: &'static str,
    description: &'static str,
    regex: &'static str,
    context_sensitive: bool,
}

const PATTERNS: &[AntiPattern] = &[
    AntiPattern {
        name: "todo_placeholder",
        description: "TODO/FIXME/HACK/XXX placeholder left in code",
        regex: r"(?i)\b(TODO|FIXME|HACK|XXX)\b",
        context_sensitive: false,
    },
    AntiPattern {
        name: "debug_print",
        description: "Debug print statement left in code",
        regex: r"^\s*(print\(|console\.log\(|fmt\.Print|System\.out\.print|puts\s|p\s|dbg!\(|println!\()",
        context_sensitive: true,
    },
    AntiPattern {
        name: "commented_code",
        description: "Commented-out code block",
        regex: r"^\s*(//|#)\s*(if|for|while|def|fn |func |class |import|from|return|var |let |const )",
        context_sensitive: false,
    },
    AntiPattern {
        name: "bare_except",
        description: "Bare except/catch catches everything",
        regex: r"^\s*(except\s*:|catch\s*\{|catch\s*\(.*Exception\s*\))",
        context_sensitive: false,
    },
    AntiPattern {
        name: "star_import",
        description: "Star/wildcard import reduces clarity",
        regex: r"from\s+\S+\s+import\s+\*|import\s+\*",
        context_sensitive: false,
    },
    AntiPattern {
        name: "magic_number",
        description: "Magic number in logic (not 0, 1, 2)",
        regex: r"[=<>!]+\s*\d{3,}|if.*\b\d{3,}\b",
        context_sensitive: false,
    },
    AntiPattern {
        name: "deeply_nested_callback",
        description: "Callback nesting exceeds readable depth",
        regex: r"^\s{16,}(if|for|while|\.then|\.catch|async|await)",
        context_sensitive: false,
    },
    AntiPattern {
        name: "empty_catch",
        description: "Empty exception handler swallows errors",
        regex: r"(except.*:\s*$|catch\s*\(.*\)\s*\{\s*\})",
        context_sensitive: false,
    },
];

const OUTPUT_STEMS: &[&str] = &[
    "main", "cli", "cmd", "output", "display", "print", "printer",
    "console", "log", "logger", "logging", "format", "formatter",
    "render", "ui", "view", "app",
];

const TEST_PARENT_DIRS: &[&str] = &[
    "test", "tests", "spec", "specs", "__tests__",
];

/// Check if a file is likely a test file based on its name and immediate parent
/// directory. Unlike `scanner::stats::is_test_file`, this avoids matching on deep
/// path components so it works correctly when the scanned project itself lives
/// under a directory named "tests" (e.g. test fixtures).
fn is_likely_test_context(path: &Path) -> bool {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_spec")
        || stem.ends_with(".test")
        || stem.ends_with(".spec")
        || stem == "conftest"
    {
        return true;
    }

    if let Some(parent) = path.parent() {
        if let Some(dir_name) = parent.file_name().and_then(|n| n.to_str()) {
            let dir_lower = dir_name.to_lowercase();
            if TEST_PARENT_DIRS.iter().any(|&d| dir_lower == d) {
                return true;
            }
        }
    }

    false
}

fn should_skip_context_sensitive(path: &Path) -> bool {
    if is_likely_test_context(path) {
        return true;
    }

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    OUTPUT_STEMS.iter().any(|&s| stem == s)
}

pub fn detect_patterns(files: &[&FileEntry]) -> Vec<PatternMatch> {
    let compiled: Vec<(&AntiPattern, Regex)> = PATTERNS
        .iter()
        .filter_map(|p| Regex::new(p.regex).ok().map(|r| (p, r)))
        .collect();

    let mut matches = Vec::new();

    for file in files {
        let skip_context = should_skip_context_sensitive(&file.path);

        for (line_num, line) in file.content.lines().enumerate() {
            for (pattern, regex) in &compiled {
                if pattern.context_sensitive && skip_context {
                    continue;
                }
                if regex.is_match(line) {
                    matches.push(PatternMatch {
                        file: file.path.clone(),
                        line: line_num + 1,
                        pattern_name: pattern.name.to_string(),
                        description: pattern.description.to_string(),
                    });
                }
            }
        }
    }

    matches
}

pub fn count_global_mutables(files: &[&FileEntry]) -> usize {
    let patterns = [
        Regex::new(r"^[A-Z_]{2,}\s*=\s*\[").unwrap(), // GLOBAL = [...]
        Regex::new(r"^[A-Z_]{2,}\s*=\s*\{").unwrap(), // GLOBAL = {...}
        Regex::new(r"^static\s+mut\s").unwrap(),       // Rust static mut
    ];

    let js_var = Regex::new(r"^var\s+\w+\s*=").unwrap();

    let mut count = 0;
    for file in files {
        if is_likely_test_context(&file.path) {
            continue;
        }

        for line in file.content.lines() {
            if line.is_empty()
                || line.starts_with("//")
                || line.starts_with('#')
                || line.starts_with(' ')
                || line.starts_with('\t')
            {
                continue;
            }

            if js_var.is_match(line) {
                count += 1;
                continue;
            }

            for pattern in &patterns {
                if pattern.is_match(line) {
                    count += 1;
                    break;
                }
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_file(content: &str) -> FileEntry {
        make_file_at("src/engine.py", content)
    }

    fn make_file_at(path: &str, content: &str) -> FileEntry {
        let p = PathBuf::from(path);
        let lang = crate::scanner::language::detect(&p);
        FileEntry {
            path: p,
            language: lang,
            size_bytes: content.len() as u64,
            line_count: content.lines().count(),
            content: content.to_string(),
        }
    }

    #[test]
    fn detects_todo() {
        let file = make_file("# TODO: fix this\nx = 1\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.iter().any(|m| m.pattern_name == "todo_placeholder"));
    }

    #[test]
    fn detects_fixme() {
        let file = make_file("// FIXME: broken\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.iter().any(|m| m.pattern_name == "todo_placeholder"));
    }

    #[test]
    fn detects_debug_print_python() {
        let file = make_file("print('debug')\nx = 1\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.iter().any(|m| m.pattern_name == "debug_print"));
    }

    #[test]
    fn detects_console_log() {
        let file = make_file_at("src/utils.js", "  console.log('test')\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.iter().any(|m| m.pattern_name == "debug_print"));
    }

    #[test]
    fn detects_bare_except() {
        let file = make_file("try:\n    pass\nexcept:\n    pass\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.iter().any(|m| m.pattern_name == "bare_except"));
    }

    #[test]
    fn detects_star_import() {
        let file = make_file("from os import *\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.iter().any(|m| m.pattern_name == "star_import"));
    }

    #[test]
    fn detects_commented_code() {
        let file = make_file("# if x > 0:\n#     return True\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.iter().any(|m| m.pattern_name == "commented_code"));
    }

    #[test]
    fn clean_code_no_patterns() {
        let file = make_file("def add(a, b):\n    return a + b\n");
        let matches = detect_patterns(&[&file]);
        assert!(matches.is_empty());
    }

    // --- Context-sensitive exclusion tests ---

    #[test]
    fn skips_debug_print_in_test_files() {
        let file = make_file_at("tests/test_utils.py", "print('debug output')\n");
        let matches = detect_patterns(&[&file]);
        assert!(
            !matches.iter().any(|m| m.pattern_name == "debug_print"),
            "Should not flag print in test files"
        );
    }

    #[test]
    fn skips_debug_print_in_test_directory() {
        let file = make_file_at("src/tests/helpers.py", "print('test helper')\n");
        let matches = detect_patterns(&[&file]);
        assert!(
            !matches.iter().any(|m| m.pattern_name == "debug_print"),
            "Should not flag print in immediate test directories"
        );
    }

    #[test]
    fn skips_debug_print_in_main_file() {
        let file = make_file_at("src/main.py", "print('Hello, world!')\n");
        let matches = detect_patterns(&[&file]);
        assert!(
            !matches.iter().any(|m| m.pattern_name == "debug_print"),
            "Should not flag print in main entry point"
        );
    }

    #[test]
    fn skips_debug_print_in_cli_module() {
        let file = make_file_at("src/cli.rs", "println!(\"Usage: tool [options]\");\n");
        let matches = detect_patterns(&[&file]);
        assert!(
            !matches.iter().any(|m| m.pattern_name == "debug_print"),
            "Should not flag println! in CLI modules"
        );
    }

    #[test]
    fn skips_debug_print_in_output_module() {
        let file = make_file_at("src/output.rs", "println!(\"{}\", result);\n");
        let matches = detect_patterns(&[&file]);
        assert!(
            !matches.iter().any(|m| m.pattern_name == "debug_print"),
            "Should not flag println! in output-focused modules"
        );
    }

    #[test]
    fn still_detects_debug_print_in_library_code() {
        let file = make_file_at("src/parser.py", "print('debug')\n");
        let matches = detect_patterns(&[&file]);
        assert!(
            matches.iter().any(|m| m.pattern_name == "debug_print"),
            "Should still flag print in regular library code"
        );
    }

    #[test]
    fn still_detects_todo_in_test_files() {
        let file = make_file_at("tests/test_utils.py", "# TODO: fix flaky test\nprint('ok')\n");
        let matches = detect_patterns(&[&file]);
        assert!(
            matches.iter().any(|m| m.pattern_name == "todo_placeholder"),
            "Non-context-sensitive patterns should still fire in test files"
        );
        assert!(
            !matches.iter().any(|m| m.pattern_name == "debug_print"),
            "But debug_print should be skipped in test files"
        );
    }

    // --- Global mutable tests ---

    #[test]
    fn global_mutable_detection() {
        let file = make_file("GLOBAL_LIST = [\n    1, 2, 3\n]\nGLOBAL_MAP = {\n}\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 2);
    }

    #[test]
    fn global_mutable_ignores_indented() {
        let file = make_file("    var x = 1\n    let y = 2\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 0);
    }

    #[test]
    fn global_mutable_var_toplevel() {
        let file = make_file("var globalState = {}\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 1);
    }

    #[test]
    fn global_mutable_static_mut() {
        let file = make_file("static mut COUNTER: u32 = 0;\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 1);
    }

    #[test]
    fn no_global_mutables_in_clean_code() {
        let file = make_file("fn main() {\n    let x = 1;\n}\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 0);
    }

    #[test]
    fn lazy_static_not_flagged_as_global_mutable() {
        let file = make_file_at("src/lib.rs", "lazy_static! {\n    static ref RE: Regex = Regex::new(r\"\\d+\").unwrap();\n}\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 0, "lazy_static should not be flagged as global mutable");
    }

    #[test]
    fn once_cell_not_flagged_as_global_mutable() {
        let file = make_file_at("src/lib.rs", "once_cell::sync::Lazy::new(|| Regex::new(r\"\\d+\").unwrap());\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 0, "once_cell should not be flagged as global mutable");
    }

    #[test]
    fn global_mutables_skipped_in_test_files() {
        let file = make_file_at("src/test_engine.py", "GLOBAL_STATE = []\nCACHE = {}\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 0, "Global mutables in test files should be skipped");
    }

    #[test]
    fn global_mutables_skipped_in_test_directories() {
        let file = make_file_at("tests/conftest.py", "FIXTURES = []\n");
        let count = count_global_mutables(&[&file]);
        assert_eq!(count, 0, "Global mutables in test directories should be skipped");
    }
}
