use regex::Regex;

use crate::scanner::FileEntry;

use super::PatternMatch;

struct AntiPattern {
    name: &'static str,
    description: &'static str,
    regex: &'static str,
}

const PATTERNS: &[AntiPattern] = &[
    AntiPattern {
        name: "todo_placeholder",
        description: "TODO/FIXME/HACK/XXX placeholder left in code",
        regex: r"(?i)\b(TODO|FIXME|HACK|XXX)\b",
    },
    AntiPattern {
        name: "debug_print",
        description: "Debug print statement left in code",
        regex: r"^\s*(print\(|console\.log\(|fmt\.Print|System\.out\.print|puts\s|p\s|dbg!\(|println!\()",
    },
    AntiPattern {
        name: "commented_code",
        description: "Commented-out code block",
        regex: r"^\s*(//|#)\s*(if|for|while|def|fn |func |class |import|from|return|var |let |const )",
    },
    AntiPattern {
        name: "bare_except",
        description: "Bare except/catch catches everything",
        regex: r"^\s*(except\s*:|catch\s*\{|catch\s*\(.*Exception\s*\))",
    },
    AntiPattern {
        name: "star_import",
        description: "Star/wildcard import reduces clarity",
        regex: r"from\s+\S+\s+import\s+\*|import\s+\*",
    },
    AntiPattern {
        name: "magic_number",
        description: "Magic number in logic (not 0, 1, 2)",
        regex: r"[=<>!]+\s*\d{3,}|if.*\b\d{3,}\b",
    },
    AntiPattern {
        name: "deeply_nested_callback",
        description: "Callback nesting exceeds readable depth",
        regex: r"^\s{16,}(if|for|while|\.then|\.catch|async|await)",
    },
    AntiPattern {
        name: "empty_catch",
        description: "Empty exception handler swallows errors",
        regex: r"(except.*:\s*$|catch\s*\(.*\)\s*\{\s*\})",
    },
];

pub fn detect_patterns(files: &[&FileEntry]) -> Vec<PatternMatch> {
    let compiled: Vec<(&AntiPattern, Regex)> = PATTERNS
        .iter()
        .filter_map(|p| Regex::new(p.regex).ok().map(|r| (p, r)))
        .collect();

    let mut matches = Vec::new();

    for file in files {
        for (line_num, line) in file.content.lines().enumerate() {
            for (pattern, regex) in &compiled {
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
        Regex::new(r"^static\s+mut\s").unwrap(),      // Rust static mut
        Regex::new(r"^(lazy_static|once_cell)").unwrap(), // Rust global patterns
    ];

    // JS/TS module-scope var (no indentation = top-level)
    let js_var = Regex::new(r"^var\s+\w+\s*=").unwrap();

    let mut count = 0;
    for file in files {
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
    use crate::scanner::language::Language;
    use std::path::PathBuf;

    fn make_file(content: &str) -> FileEntry {
        FileEntry {
            path: PathBuf::from("test.py"),
            language: Language::Python,
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
        let file = make_file("  console.log('test')\n");
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
}
