use crate::scanner::language::Language;
use crate::scanner::FileEntry;
use tree_sitter::{Parser, Tree};

pub fn parse_file(file: &FileEntry) -> Option<Tree> {
    let mut parser = Parser::new();
    let ts_language = get_language(file.language)?;
    parser.set_language(&ts_language).ok()?;
    parser.parse(&file.content, None)
}

fn get_language(lang: Language) -> Option<tree_sitter::Language> {
    match lang {
        Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
        Language::JavaScript | Language::Jsx => Some(tree_sitter_javascript::LANGUAGE.into()),
        Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Language::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
        Language::Java => Some(tree_sitter_java::LANGUAGE.into()),
        Language::C => Some(tree_sitter_c::LANGUAGE.into()),
        Language::Cpp => Some(tree_sitter_cpp::LANGUAGE.into()),
        Language::Ruby => Some(tree_sitter_ruby::LANGUAGE.into()),
        _ => None,
    }
}

pub fn node_text<'a>(node: &tree_sitter::Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

pub fn count_node_kind(node: &tree_sitter::Node, kinds: &[&str]) -> usize {
    let mut count = 0;
    let mut cursor = node.walk();

    fn walk_count(cursor: &mut tree_sitter::TreeCursor, kinds: &[&str], count: &mut usize) {
        if kinds.contains(&cursor.node().kind()) {
            *count += 1;
        }
        if cursor.goto_first_child() {
            loop {
                walk_count(cursor, kinds, count);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    walk_count(&mut cursor, kinds, &mut count);
    count
}

pub fn max_depth(node: &tree_sitter::Node, nesting_kinds: &[&str]) -> usize {
    fn walk_depth(
        cursor: &mut tree_sitter::TreeCursor,
        nesting_kinds: &[&str],
        current: usize,
        max: &mut usize,
    ) {
        let depth = if nesting_kinds.contains(&cursor.node().kind()) {
            current + 1
        } else {
            current
        };
        if depth > *max {
            *max = depth;
        }
        if cursor.goto_first_child() {
            loop {
                walk_depth(cursor, nesting_kinds, depth, max);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    let mut max = 0;
    let mut cursor = node.walk();
    walk_depth(&mut cursor, nesting_kinds, 0, &mut max);
    max
}
