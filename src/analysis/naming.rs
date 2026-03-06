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
