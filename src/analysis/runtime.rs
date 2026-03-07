use crate::scanner::language::Language;
use crate::scanner::FileEntry;

use super::tree_sitter_util;

#[derive(Debug)]
pub struct RuntimeHazard {
    pub kind: HazardKind,
    pub file: std::path::PathBuf,
    pub line: usize,
}

#[derive(Debug, PartialEq)]
pub enum HazardKind {
    ModuleLevelCall,
    SingletonPattern,
    MutableClassVariable,
    GlobalEventListener,
}

pub fn detect_runtime_hazards(tree: &tree_sitter::Tree, file: &FileEntry) -> Vec<RuntimeHazard> {
    let mut hazards = Vec::new();
    let root = tree.root_node();

    match file.language {
        Language::Python => detect_python_hazards(&root, &file.content, file, &mut hazards),
        Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx => {
            detect_js_hazards(&root, &file.content, file, &mut hazards)
        }
        _ => {}
    }

    hazards
}

fn detect_python_hazards(
    root: &tree_sitter::Node,
    source: &str,
    file: &FileEntry,
    hazards: &mut Vec<RuntimeHazard>,
) {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return;
    }

    loop {
        let node = cursor.node();
        match node.kind() {
            "expression_statement" => {
                if let Some(child) = node.child(0) {
                    if child.kind() == "call" {
                        let text = tree_sitter_util::node_text(&child, source);
                        if !is_python_safe_toplevel_call(text) {
                            hazards.push(RuntimeHazard {
                                kind: HazardKind::ModuleLevelCall,
                                file: file.path.clone(),
                                line: node.start_position().row + 1,
                            });
                        }
                    }
                }
            }
            "class_definition" => {
                detect_python_class_hazards(&node, source, file, hazards);
            }
            _ => {}
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

fn is_python_safe_toplevel_call(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("print(")
        || trimmed.starts_with("logging.")
        || trimmed.starts_with("logger.")
        || trimmed.starts_with("warnings.")
        || trimmed.starts_with("register(")
        || trimmed.starts_with("app =")
}

fn detect_python_class_hazards(
    class_node: &tree_sitter::Node,
    source: &str,
    file: &FileEntry,
    hazards: &mut Vec<RuntimeHazard>,
) {
    let mut has_instance_field = false;
    let mut has_get_instance = false;

    if let Some(body) = class_node.child_by_field_name("body") {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                match node.kind() {
                    "expression_statement" => {
                        if let Some(child) = node.child(0) {
                            if child.kind() == "assignment" {
                                let text = tree_sitter_util::node_text(&child, source);
                                if text.contains("_instance") {
                                    has_instance_field = true;
                                } else if !text.contains(":") {
                                    hazards.push(RuntimeHazard {
                                        kind: HazardKind::MutableClassVariable,
                                        file: file.path.clone(),
                                        line: node.start_position().row + 1,
                                    });
                                }
                            }
                        }
                    }
                    "function_definition" => {
                        if let Some(name) = node.child_by_field_name("name") {
                            let fn_name = tree_sitter_util::node_text(&name, source);
                            if fn_name == "get_instance"
                                || fn_name == "getInstance"
                                || fn_name == "instance"
                            {
                                has_get_instance = true;
                            }
                        }
                    }
                    "decorated_definition" => {
                        let text = tree_sitter_util::node_text(&node, source);
                        if text.contains("get_instance")
                            || text.contains("getInstance")
                            || text.contains("fn instance")
                            || text.contains("def instance")
                        {
                            has_get_instance = true;
                        }
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    if has_instance_field && has_get_instance {
        hazards.push(RuntimeHazard {
            kind: HazardKind::SingletonPattern,
            file: file.path.clone(),
            line: class_node.start_position().row + 1,
        });
    }
}

fn detect_js_hazards(
    root: &tree_sitter::Node,
    source: &str,
    file: &FileEntry,
    hazards: &mut Vec<RuntimeHazard>,
) {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return;
    }

    loop {
        let node = cursor.node();
        match node.kind() {
            "expression_statement" => {
                let text = tree_sitter_util::node_text(&node, source);
                if is_js_event_listener(text) {
                    hazards.push(RuntimeHazard {
                        kind: HazardKind::GlobalEventListener,
                        file: file.path.clone(),
                        line: node.start_position().row + 1,
                    });
                } else if is_js_toplevel_side_effect(text) {
                    hazards.push(RuntimeHazard {
                        kind: HazardKind::ModuleLevelCall,
                        file: file.path.clone(),
                        line: node.start_position().row + 1,
                    });
                }
            }
            "class_declaration" => {
                detect_js_class_hazards(&node, source, file, hazards);
            }
            _ => {}
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

fn is_js_event_listener(text: &str) -> bool {
    let t = text.trim();
    t.contains("addEventListener(")
        || t.contains(".on(")
        || t.contains(".subscribe(")
        || t.contains(".listen(")
}

fn is_js_toplevel_side_effect(text: &str) -> bool {
    let t = text.trim();
    t.contains("app.use(")
        || t.contains("router.use(")
        || t.contains("app.get(")
        || t.contains("app.post(")
        || t.contains("app.put(")
        || t.contains("app.delete(")
}

fn detect_js_class_hazards(
    class_node: &tree_sitter::Node,
    source: &str,
    file: &FileEntry,
    hazards: &mut Vec<RuntimeHazard>,
) {
    let mut has_instance_field = false;
    let mut has_get_instance = false;

    if let Some(body) = class_node.child_by_field_name("body") {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let node = cursor.node();
                let text = tree_sitter_util::node_text(&node, source);

                if node.kind() == "method_definition" || node.kind() == "public_field_definition" {
                    if text.contains("getInstance") || text.contains("instance") {
                        has_get_instance = true;
                    }
                    if text.contains("_instance") {
                        has_instance_field = true;
                    }
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    if has_instance_field && has_get_instance {
        hazards.push(RuntimeHazard {
            kind: HazardKind::SingletonPattern,
            file: file.path.clone(),
            line: class_node.start_position().row + 1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse_and_detect(code: &str, lang: Language) -> Vec<RuntimeHazard> {
        let file = FileEntry {
            path: PathBuf::from("test.py"),
            content: code.to_string(),
            line_count: code.lines().count(),
            size_bytes: code.len() as u64,
            language: lang,
        };
        if let Some(tree) = tree_sitter_util::parse_file(&file) {
            detect_runtime_hazards(&tree, &file)
        } else {
            vec![]
        }
    }

    #[test]
    fn python_module_level_call() {
        let code = "setup_database()\nx = 42\n";
        let hazards = parse_and_detect(code, Language::Python);
        assert!(
            hazards
                .iter()
                .any(|h| h.kind == HazardKind::ModuleLevelCall),
            "Should detect top-level function call"
        );
    }

    #[test]
    fn python_safe_toplevel_not_flagged() {
        let code = "print(\"hello\")\n";
        let hazards = parse_and_detect(code, Language::Python);
        assert!(
            !hazards
                .iter()
                .any(|h| h.kind == HazardKind::ModuleLevelCall),
            "print() should not be flagged"
        );
    }

    #[test]
    fn python_singleton_detected() {
        let code = r#"
class Database:
    _instance = None

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance
"#;
        let hazards = parse_and_detect(code, Language::Python);
        assert!(
            hazards
                .iter()
                .any(|h| h.kind == HazardKind::SingletonPattern),
            "Should detect singleton pattern"
        );
    }

    #[test]
    fn python_mutable_class_variable() {
        let code = r#"
class Config:
    items = []
    count = 0
"#;
        let hazards = parse_and_detect(code, Language::Python);
        assert!(
            hazards
                .iter()
                .any(|h| h.kind == HazardKind::MutableClassVariable),
            "Should detect mutable class variables"
        );
    }

    #[test]
    fn js_event_listener() {
        let code = "document.addEventListener('click', handler);\n";
        let hazards = parse_and_detect(code, Language::JavaScript);
        assert!(
            hazards
                .iter()
                .any(|h| h.kind == HazardKind::GlobalEventListener),
            "Should detect global event listener"
        );
    }

    #[test]
    fn js_express_route_registration() {
        let code = "app.use(middleware);\napp.get('/api', handler);\n";
        let hazards = parse_and_detect(code, Language::JavaScript);
        let side_effects: Vec<_> = hazards
            .iter()
            .filter(|h| h.kind == HazardKind::ModuleLevelCall)
            .collect();
        assert!(
            side_effects.len() >= 2,
            "Should detect app.use and app.get as side effects, got {}",
            side_effects.len()
        );
    }

    #[test]
    fn clean_code_no_hazards() {
        let code = r#"
def add(a, b):
    return a + b

class Calculator:
    def __init__(self):
        self.history = []
"#;
        let hazards = parse_and_detect(code, Language::Python);
        assert!(hazards.is_empty(), "Clean code should have no hazards");
    }
}
