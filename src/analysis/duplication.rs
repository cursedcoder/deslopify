use std::collections::{HashMap, HashSet};

use regex::Regex;

use crate::scanner::FileEntry;

use super::DuplicateCluster;

const MIN_CHUNK_LINES: usize = 8;

pub fn find_duplicates(files: &[&FileEntry]) -> Vec<DuplicateCluster> {
    let mut normalized_map: HashMap<u64, Vec<(std::path::PathBuf, usize, String)>> = HashMap::new();

    for file in files {
        let lines: Vec<&str> = file.content.lines().collect();
        if lines.len() < MIN_CHUNK_LINES {
            continue;
        }

        for start in 0..lines.len().saturating_sub(MIN_CHUNK_LINES) {
            let raw_chunk: String = lines[start..start + MIN_CHUNK_LINES]
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");

            if raw_chunk.len() < 40 {
                continue;
            }

            let normalized = normalize_chunk(&raw_chunk);
            if normalized.len() < 20 {
                continue;
            }

            let hash = simple_hash(&normalized);
            normalized_map
                .entry(hash)
                .or_default()
                .push((file.path.clone(), start + 1, raw_chunk));
        }
    }

    normalized_map
        .into_iter()
        .filter(|(_, locations)| {
            if locations.len() < 2 {
                return false;
            }
            let unique_files: HashSet<_> = locations.iter().map(|(p, _, _)| p).collect();
            unique_files.len() >= 2
        })
        .map(|(hash, locations)| {
            let similarity = compute_similarity(&locations);
            let locs = locations
                .into_iter()
                .map(|(path, line, _)| (path, line))
                .collect();
            DuplicateCluster {
                hash,
                locations: locs,
                line_count: MIN_CHUNK_LINES,
                similarity,
            }
        })
        .collect()
}

fn normalize_chunk(chunk: &str) -> String {
    let re_string = Regex::new(r#"("[^"]*"|'[^']*')"#).unwrap();
    let re_ident = Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap();

    let without_strings = re_string.replace_all(chunk, "\"_\"");

    let without_idents = re_ident.replace_all(&without_strings, |caps: &regex::Captures| {
        let word = caps.get(0).unwrap().as_str();
        if is_keyword(word) {
            word.to_string()
        } else {
            "_ID_".to_string()
        }
    });

    without_idents
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "if" | "else"
            | "for"
            | "while"
            | "return"
            | "fn"
            | "def"
            | "class"
            | "struct"
            | "enum"
            | "match"
            | "let"
            | "const"
            | "var"
            | "import"
            | "from"
            | "use"
            | "pub"
            | "self"
            | "Self"
            | "true"
            | "false"
            | "None"
            | "null"
            | "nil"
            | "async"
            | "await"
            | "try"
            | "catch"
            | "except"
            | "finally"
            | "raise"
            | "throw"
            | "new"
            | "in"
            | "not"
            | "and"
            | "or"
            | "is"
            | "as"
            | "with"
            | "yield"
            | "break"
            | "continue"
            | "pass"
            | "lambda"
            | "function"
            | "export"
            | "default"
            | "static"
            | "mut"
            | "impl"
            | "trait"
            | "type"
            | "interface"
            | "extends"
            | "super"
            | "package"
            | "void"
            | "int"
            | "float"
            | "double"
            | "bool"
            | "string"
            | "char"
    )
}

fn compute_similarity(locations: &[(std::path::PathBuf, usize, String)]) -> f64 {
    if locations.len() < 2 {
        return 1.0;
    }
    let first_tokens: HashSet<&str> = locations[0].2.split_whitespace().collect();
    let mut min_jaccard = 1.0f64;

    for loc in &locations[1..] {
        let other_tokens: HashSet<&str> = loc.2.split_whitespace().collect();
        let intersection = first_tokens.intersection(&other_tokens).count();
        let union = first_tokens.union(&other_tokens).count();
        if union > 0 {
            let jaccard = intersection as f64 / union as f64;
            min_jaccard = min_jaccard.min(jaccard);
        }
    }
    min_jaccard
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_identifiers() {
        let code = "let foo = bar + baz;";
        let normalized = normalize_chunk(code);
        assert_eq!(normalized, "let _ID_ = _ID_ + _ID_;");
    }

    #[test]
    fn normalize_strips_strings() {
        let code = r#"print("hello world")"#;
        let normalized = normalize_chunk(code);
        assert!(!normalized.contains("hello"));
        assert!(!normalized.contains("world"));
    }

    #[test]
    fn normalize_preserves_keywords() {
        let code = "if x > 0 { return true; }";
        let normalized = normalize_chunk(code);
        assert!(normalized.contains("if"));
        assert!(normalized.contains("return"));
        assert!(normalized.contains("true"));
    }

    #[test]
    fn identical_code_detected() {
        use std::path::PathBuf;
        let content = "line1\nline2\nline3\nline4\nlet x = foo(bar);\nlet y = baz(qux);\nresult = x + y;\nreturn result;\nline9\n";
        let files = vec![
            FileEntry {
                path: PathBuf::from("a.rs"),
                content: content.to_string(),
                line_count: 9,
                size_bytes: content.len() as u64,
                language: crate::scanner::language::Language::Rust,
            },
            FileEntry {
                path: PathBuf::from("b.rs"),
                content: content.to_string(),
                line_count: 9,
                size_bytes: content.len() as u64,
                language: crate::scanner::language::Language::Rust,
            },
        ];
        let refs: Vec<&FileEntry> = files.iter().collect();
        let dupes = find_duplicates(&refs);
        assert!(!dupes.is_empty(), "identical code should be detected");
        assert!(
            dupes[0].similarity > 0.99,
            "identical code should have similarity ~1.0"
        );
    }

    #[test]
    fn renamed_variables_detected() {
        use std::path::PathBuf;
        let content_a = "line1\nline2\nline3\nline4\nlet alpha = compute(beta);\nlet gamma = process(delta);\nresult = alpha + gamma;\nreturn result;\nline9\n";
        let content_b = "line1\nline2\nline3\nline4\nlet foo = compute(bar);\nlet baz = process(qux);\nresult = foo + baz;\nreturn result;\nline9\n";
        let files = vec![
            FileEntry {
                path: PathBuf::from("a.rs"),
                content: content_a.to_string(),
                line_count: 9,
                size_bytes: content_a.len() as u64,
                language: crate::scanner::language::Language::Rust,
            },
            FileEntry {
                path: PathBuf::from("b.rs"),
                content: content_b.to_string(),
                line_count: 9,
                size_bytes: content_b.len() as u64,
                language: crate::scanner::language::Language::Rust,
            },
        ];
        let refs: Vec<&FileEntry> = files.iter().collect();
        let dupes = find_duplicates(&refs);
        assert!(
            !dupes.is_empty(),
            "renamed variables should still be detected as duplicates"
        );
    }

    #[test]
    fn different_structure_not_matched() {
        use std::path::PathBuf;
        let content_a = "aaa\nbbb\nccc\nddd\neee\nfff\nggg\nhhh\niii\n";
        let content_b = "zzz\nyyy\nxxx\nwww\nvvv\nuuu\nttt\nsss\nrrr\n";
        let files = vec![
            FileEntry {
                path: PathBuf::from("a.rs"),
                content: content_a.to_string(),
                line_count: 9,
                size_bytes: content_a.len() as u64,
                language: crate::scanner::language::Language::Rust,
            },
            FileEntry {
                path: PathBuf::from("b.rs"),
                content: content_b.to_string(),
                line_count: 9,
                size_bytes: content_b.len() as u64,
                language: crate::scanner::language::Language::Rust,
            },
        ];
        let refs: Vec<&FileEntry> = files.iter().collect();
        let dupes = find_duplicates(&refs);
        assert!(
            dupes.is_empty(),
            "structurally different code should not match"
        );
    }
}
