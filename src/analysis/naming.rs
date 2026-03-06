use super::{FunctionInfo, NamingStats};
use regex::Regex;

pub fn analyze_naming(functions: &[FunctionInfo]) -> NamingStats {
    let mut stats = NamingStats {
        snake_case_count: 0,
        camel_case_count: 0,
        pascal_case_count: 0,
        screaming_snake_count: 0,
        mixed_count: 0,
    };

    let snake = Regex::new(r"^[a-z][a-z0-9]*(_[a-z0-9]+)*$").unwrap();
    let camel = Regex::new(r"^[a-z][a-zA-Z0-9]*$").unwrap();
    let pascal = Regex::new(r"^[A-Z][a-zA-Z0-9]*$").unwrap();
    let screaming = Regex::new(r"^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$").unwrap();

    for func in functions {
        let name = &func.name;
        if name == "<anonymous>" || name.is_empty() {
            continue;
        }

        if snake.is_match(name) {
            stats.snake_case_count += 1;
        } else if screaming.is_match(name) {
            stats.screaming_snake_count += 1;
        } else if pascal.is_match(name) {
            stats.pascal_case_count += 1;
        } else if camel.is_match(name) {
            stats.camel_case_count += 1;
        } else {
            stats.mixed_count += 1;
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_fn(name: &str) -> FunctionInfo {
        FunctionInfo {
            name: name.to_string(),
            file: PathBuf::from("test.py"),
            start_line: 1,
            line_count: 10,
            cyclomatic_complexity: 1,
            max_nesting: 0,
        }
    }

    #[test]
    fn all_snake_case() {
        let funcs = vec![
            make_fn("get_user"),
            make_fn("process_data"),
            make_fn("validate"),
        ];
        let stats = analyze_naming(&funcs);
        assert_eq!(stats.snake_case_count, 3);
        assert_eq!(stats.camel_case_count, 0);
        assert!((stats.dominant_style_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn all_camel_case() {
        let funcs = vec![
            make_fn("getUser"),
            make_fn("processData"),
            make_fn("validateInput"),
        ];
        let stats = analyze_naming(&funcs);
        assert_eq!(stats.camel_case_count, 3);
        assert_eq!(stats.snake_case_count, 0);
    }

    #[test]
    fn mixed_naming() {
        let funcs = vec![
            make_fn("get_user"),
            make_fn("processData"),
            make_fn("ValidateInput"),
        ];
        let stats = analyze_naming(&funcs);
        assert_eq!(stats.snake_case_count, 1);
        assert_eq!(stats.camel_case_count, 1);
        assert_eq!(stats.pascal_case_count, 1);
        assert!((stats.dominant_style_ratio() - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn anonymous_skipped() {
        let funcs = vec![make_fn("<anonymous>"), make_fn(""), make_fn("valid_name")];
        let stats = analyze_naming(&funcs);
        assert_eq!(stats.total(), 1);
        assert_eq!(stats.snake_case_count, 1);
    }

    #[test]
    fn screaming_snake() {
        let funcs = vec![make_fn("MAX_SIZE"), make_fn("DEFAULT_TIMEOUT")];
        let stats = analyze_naming(&funcs);
        assert_eq!(stats.screaming_snake_count, 2);
    }

    #[test]
    fn empty_input() {
        let stats = analyze_naming(&[]);
        assert_eq!(stats.total(), 0);
        assert!((stats.dominant_style_ratio() - 1.0).abs() < f64::EPSILON);
    }
}
