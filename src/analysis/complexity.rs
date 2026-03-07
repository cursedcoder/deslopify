use crate::scanner::language::Language;
use crate::scanner::FileEntry;

use super::tree_sitter_util;
use super::FunctionInfo;

pub fn extract_functions(tree: &tree_sitter::Tree, file: &FileEntry) -> Vec<FunctionInfo> {
    let root = tree.root_node();
    let function_kinds = get_function_kinds(file.language);
    let branch_kinds = get_branch_kinds(file.language);
    let nesting_kinds = get_nesting_kinds(file.language);
    let mut functions = Vec::new();

    collect_functions(
        &root,
        &file.content,
        file,
        &function_kinds,
        &branch_kinds,
        &nesting_kinds,
        &mut functions,
    );

    functions
}

fn collect_functions(
    node: &tree_sitter::Node,
    source: &str,
    file: &FileEntry,
    function_kinds: &[&str],
    branch_kinds: &[&str],
    nesting_kinds: &[&str],
    out: &mut Vec<FunctionInfo>,
) {
    if function_kinds.contains(&node.kind()) {
        let name = extract_function_name(node, source, file.language);
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        let line_count = end_line.saturating_sub(start_line) + 1;
        let cyclomatic_complexity = 1 + tree_sitter_util::count_node_kind(node, branch_kinds);
        let max_nesting = tree_sitter_util::max_depth(node, nesting_kinds);

        out.push(FunctionInfo {
            name,
            file: file.path.clone(),
            start_line,
            line_count,
            cyclomatic_complexity,
            max_nesting,
        });
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_functions(
                &cursor.node(),
                source,
                file,
                function_kinds,
                branch_kinds,
                nesting_kinds,
                out,
            );
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn extract_function_name(node: &tree_sitter::Node, source: &str, lang: Language) -> String {
    let name_field = match lang {
        Language::Go => "name",
        _ => "name",
    };

    if let Some(name_node) = node.child_by_field_name(name_field) {
        return tree_sitter_util::node_text(&name_node, source).to_string();
    }

    // Fallback: look for first identifier child
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "identifier"
                || child.kind() == "property_identifier"
                || child.kind() == "field_identifier"
            {
                return tree_sitter_util::node_text(&child, source).to_string();
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    "<anonymous>".to_string()
}

fn get_function_kinds(lang: Language) -> Vec<&'static str> {
    match lang {
        Language::Python => vec!["function_definition", "decorated_definition"],
        Language::JavaScript | Language::Jsx => vec![
            "function_declaration",
            "method_definition",
            "arrow_function",
            "function",
        ],
        Language::TypeScript | Language::Tsx => vec![
            "function_declaration",
            "method_definition",
            "arrow_function",
            "function",
        ],
        Language::Rust => vec!["function_item", "impl_item"],
        Language::Go => vec!["function_declaration", "method_declaration"],
        Language::Java => vec!["method_declaration", "constructor_declaration"],
        Language::C | Language::Cpp => vec!["function_definition"],
        Language::Ruby => vec!["method", "singleton_method"],
        Language::Php => vec![
            "function_definition",
            "method_declaration",
            "anonymous_function",
        ],
        _ => vec![],
    }
}

fn get_branch_kinds(lang: Language) -> Vec<&'static str> {
    match lang {
        Language::Python => vec![
            "if_statement",
            "elif_clause",
            "for_statement",
            "while_statement",
            "except_clause",
            "with_statement",
            "conditional_expression",
            "boolean_operator",
        ],
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => vec![
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_case",
            "catch_clause",
            "ternary_expression",
            "binary_expression",
        ],
        Language::Rust => vec![
            "if_expression",
            "match_arm",
            "for_expression",
            "while_expression",
            "loop_expression",
        ],
        Language::Go => vec![
            "if_statement",
            "for_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
        ],
        Language::Java => vec![
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "catch_clause",
            "ternary_expression",
            "switch_expression",
        ],
        Language::C | Language::Cpp => vec![
            "if_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "case_statement",
            "conditional_expression",
        ],
        Language::Ruby => vec![
            "if", "elsif", "unless", "while", "for", "case", "when", "rescue",
        ],
        Language::Php => vec![
            "if_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_block",
            "catch_clause",
            "conditional_expression",
            "match_expression",
        ],
        _ => vec![],
    }
}

fn get_nesting_kinds(lang: Language) -> Vec<&'static str> {
    match lang {
        Language::Python => vec![
            "if_statement",
            "for_statement",
            "while_statement",
            "with_statement",
            "try_statement",
        ],
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => vec![
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "switch_statement",
        ],
        Language::Rust => vec![
            "if_expression",
            "for_expression",
            "while_expression",
            "loop_expression",
            "match_expression",
        ],
        Language::Go => vec![
            "if_statement",
            "for_statement",
            "expression_switch_statement",
            "type_switch_statement",
            "select_statement",
        ],
        Language::Java => vec![
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "try_statement",
            "switch_expression",
        ],
        Language::C | Language::Cpp => vec![
            "if_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
        ],
        Language::Ruby => vec!["if", "unless", "while", "for", "case", "begin"],
        Language::Php => vec![
            "if_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
            "try_statement",
        ],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::FileEntry;
    use std::path::PathBuf;

    #[test]
    fn php_functions_extracted() {
        let code = r#"<?php
namespace App\Service;

class UserService {
    public function findById(int $id): ?User {
        return $this->em->find(User::class, $id);
    }

    public function save(User $user): void {
        $this->em->persist($user);
        $this->em->flush();
    }
}
"#;
        let file = FileEntry {
            path: PathBuf::from("src/Service/UserService.php"),
            content: code.to_string(),
            line_count: 14,
            size_bytes: code.len() as u64,
            language: Language::Php,
        };

        let tree = super::super::tree_sitter_util::parse_file(&file).expect("parse_file returned None");
        let funcs = extract_functions(&tree, &file);
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"findById"),
            "Expected findById in {:?}",
            names
        );
        assert!(
            names.contains(&"save"),
            "Expected save in {:?}",
            names
        );
        assert_eq!(funcs.len(), 2);
    }

    #[test]
    fn php_closure_detected() {
        let code = r#"<?php
$callback = function($x) {
    return $x * 2;
};
"#;
        let file = FileEntry {
            path: PathBuf::from("test.php"),
            content: code.to_string(),
            line_count: 5,
            size_bytes: code.len() as u64,
            language: Language::Php,
        };
        let tree = super::super::tree_sitter_util::parse_file(&file).unwrap();
        let funcs = extract_functions(&tree, &file);
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "<anonymous>");
    }
}
