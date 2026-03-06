use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::scanner::language::Language;
use crate::scanner::FileEntry;

use super::tree_sitter_util;
use super::ImportInfo;

pub fn extract_imports(tree: &tree_sitter::Tree, file: &FileEntry) -> Vec<ImportInfo> {
    let root = tree.root_node();
    let import_kinds = get_import_kinds(file.language);
    let mut imports = Vec::new();

    collect_imports(&root, &file.content, file, &import_kinds, &mut imports);

    imports
}

fn collect_imports(
    node: &tree_sitter::Node,
    source: &str,
    file: &FileEntry,
    import_kinds: &[&str],
    out: &mut Vec<ImportInfo>,
) {
    if import_kinds.contains(&node.kind()) {
        let module_path = extract_module_path(node, source, file.language);
        if !module_path.is_empty() {
            out.push(ImportInfo {
                file: file.path.clone(),
                module_path,
                line: node.start_position().row + 1,
            });
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_imports(&cursor.node(), source, file, import_kinds, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn extract_module_path(node: &tree_sitter::Node, source: &str, lang: Language) -> String {
    match lang {
        Language::Python => {
            if let Some(name_node) = node.child_by_field_name("module_name") {
                return tree_sitter_util::node_text(&name_node, source).to_string();
            }
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "dotted_name" || child.kind() == "identifier" {
                        return tree_sitter_util::node_text(&child, source).to_string();
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => {
            if let Some(source_node) = node.child_by_field_name("source") {
                let text = tree_sitter_util::node_text(&source_node, source);
                return text.trim_matches(|c| c == '\'' || c == '"').to_string();
            }
        }
        Language::Rust => {
            let text = tree_sitter_util::node_text(node, source);
            if let Some(start) = text.find("use ") {
                let rest = &text[start + 4..];
                if let Some(end) = rest.find(';') {
                    return rest[..end].trim().to_string();
                }
            }
        }
        Language::Go => {
            if let Some(path_node) = node.child_by_field_name("path") {
                let text = tree_sitter_util::node_text(&path_node, source);
                return text.trim_matches('"').to_string();
            }
        }
        Language::Java => {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "scoped_identifier" {
                        return tree_sitter_util::node_text(&child, source).to_string();
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        Language::C | Language::Cpp => {
            if let Some(path_node) = node.child_by_field_name("path") {
                let text = tree_sitter_util::node_text(&path_node, source);
                return text
                    .trim_matches(|c| c == '<' || c == '>' || c == '"')
                    .to_string();
            }
        }
        Language::Ruby => {
            let text = tree_sitter_util::node_text(node, source);
            return text.to_string();
        }
        _ => {}
    }
    String::new()
}

fn get_import_kinds(lang: Language) -> Vec<&'static str> {
    match lang {
        Language::Python => vec!["import_statement", "import_from_statement"],
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => {
            vec!["import_statement"]
        }
        Language::Rust => vec!["use_declaration"],
        Language::Go => vec!["import_spec"],
        Language::Java => vec!["import_declaration"],
        Language::C | Language::Cpp => vec!["preproc_include"],
        Language::Ruby => vec!["call"],
        _ => vec![],
    }
}

pub struct ImportGraphStats {
    pub avg_fan_out: f64,
    pub max_fan_out: usize,
    pub max_fan_out_file: Option<PathBuf>,
    pub avg_fan_in: f64,
    pub max_fan_in: usize,
    pub max_fan_in_module: Option<String>,
    pub circular_dep_count: usize,
}

pub fn compute_import_graph(imports: &[ImportInfo]) -> ImportGraphStats {
    let mut fan_out: HashMap<PathBuf, usize> = HashMap::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    let mut edges: HashMap<String, HashSet<String>> = HashMap::new();

    for imp in imports {
        *fan_out.entry(imp.file.clone()).or_insert(0) += 1;
        *fan_in.entry(imp.module_path.clone()).or_insert(0) += 1;

        let from = imp.file.to_string_lossy().to_string();
        edges
            .entry(from)
            .or_default()
            .insert(imp.module_path.clone());
    }

    let avg_fan_out = if fan_out.is_empty() {
        0.0
    } else {
        fan_out.values().sum::<usize>() as f64 / fan_out.len() as f64
    };
    let (max_fan_out, max_fan_out_file) = fan_out
        .iter()
        .max_by_key(|(_, &v)| v)
        .map(|(k, &v)| (v, Some(k.clone())))
        .unwrap_or((0, None));

    let avg_fan_in = if fan_in.is_empty() {
        0.0
    } else {
        fan_in.values().sum::<usize>() as f64 / fan_in.len() as f64
    };
    let (max_fan_in, max_fan_in_module) = fan_in
        .iter()
        .max_by_key(|(_, &v)| v)
        .map(|(k, &v)| (v, Some(k.clone())))
        .unwrap_or((0, None));

    let circular_dep_count = count_circular_deps(&edges);

    ImportGraphStats {
        avg_fan_out,
        max_fan_out,
        max_fan_out_file,
        avg_fan_in,
        max_fan_in,
        max_fan_in_module,
        circular_dep_count,
    }
}

fn count_circular_deps(edges: &HashMap<String, HashSet<String>>) -> usize {
    let mut count = 0;
    for (from, targets) in edges {
        for target in targets {
            if let Some(back_targets) = edges.get(target) {
                if back_targets.contains(from) {
                    count += 1;
                }
            }
        }
    }
    count / 2 // each cycle counted twice
}
