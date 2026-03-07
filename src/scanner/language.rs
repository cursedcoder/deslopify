use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Jsx,
    Rust,
    Go,
    Java,
    C,
    Cpp,
    Ruby,
    CSharp,
    Php,
    Swift,
    Kotlin,
    Scala,
    Shell,
    Lua,
    Zig,
    Dart,
    Elixir,
    Haskell,
    OCaml,
    Toml,
    Yaml,
    Json,
    Markdown,
    Html,
    Css,
    Sql,
    Unknown,
}

impl Language {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Tsx => "TSX",
            Self::Jsx => "JSX",
            Self::Rust => "Rust",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Ruby => "Ruby",
            Self::CSharp => "C#",
            Self::Php => "PHP",
            Self::Swift => "Swift",
            Self::Kotlin => "Kotlin",
            Self::Scala => "Scala",
            Self::Shell => "Shell",
            Self::Lua => "Lua",
            Self::Zig => "Zig",
            Self::Dart => "Dart",
            Self::Elixir => "Elixir",
            Self::Haskell => "Haskell",
            Self::OCaml => "OCaml",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Sql => "SQL",
            Self::Unknown => "Unknown",
        }
    }

    pub fn is_source_code(&self) -> bool {
        !matches!(
            self,
            Self::Unknown | Self::Toml | Self::Yaml | Self::Json | Self::Markdown
        )
    }

    pub fn has_tree_sitter_support(&self) -> bool {
        matches!(
            self,
            Self::Python
                | Self::JavaScript
                | Self::TypeScript
                | Self::Tsx
                | Self::Jsx
                | Self::Rust
                | Self::Go
                | Self::Java
                | Self::C
                | Self::Cpp
                | Self::Ruby
                | Self::Php
        )
    }
}

pub fn detect(path: &Path) -> Language {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "py" | "pyi" => Language::Python,
        "js" | "mjs" | "cjs" => Language::JavaScript,
        "ts" | "mts" | "cts" => Language::TypeScript,
        "tsx" => Language::Tsx,
        "jsx" => Language::Jsx,
        "rs" => Language::Rust,
        "go" => Language::Go,
        "java" => Language::Java,
        "c" | "h" => Language::C,
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Language::Cpp,
        "rb" => Language::Ruby,
        "cs" => Language::CSharp,
        "php" => Language::Php,
        "swift" => Language::Swift,
        "kt" | "kts" => Language::Kotlin,
        "scala" | "sc" => Language::Scala,
        "sh" | "bash" | "zsh" => Language::Shell,
        "lua" => Language::Lua,
        "zig" => Language::Zig,
        "dart" => Language::Dart,
        "ex" | "exs" => Language::Elixir,
        "hs" => Language::Haskell,
        "ml" | "mli" => Language::OCaml,
        "toml" => Language::Toml,
        "yml" | "yaml" => Language::Yaml,
        "json" => Language::Json,
        "md" | "mdx" => Language::Markdown,
        "html" | "htm" => Language::Html,
        "css" | "scss" | "less" => Language::Css,
        "sql" => Language::Sql,
        _ => match filename.as_str() {
            "makefile" | "gnumakefile" => Language::Shell,
            "dockerfile" => Language::Shell,
            _ => Language::Unknown,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detect_python() {
        assert_eq!(detect(&PathBuf::from("main.py")), Language::Python);
        assert_eq!(detect(&PathBuf::from("types.pyi")), Language::Python);
    }

    #[test]
    fn detect_javascript_variants() {
        assert_eq!(detect(&PathBuf::from("app.js")), Language::JavaScript);
        assert_eq!(detect(&PathBuf::from("lib.mjs")), Language::JavaScript);
        assert_eq!(detect(&PathBuf::from("config.cjs")), Language::JavaScript);
    }

    #[test]
    fn detect_typescript_variants() {
        assert_eq!(detect(&PathBuf::from("app.ts")), Language::TypeScript);
        assert_eq!(detect(&PathBuf::from("app.tsx")), Language::Tsx);
        assert_eq!(detect(&PathBuf::from("app.jsx")), Language::Jsx);
    }

    #[test]
    fn detect_systems_languages() {
        assert_eq!(detect(&PathBuf::from("main.rs")), Language::Rust);
        assert_eq!(detect(&PathBuf::from("main.go")), Language::Go);
        assert_eq!(detect(&PathBuf::from("Main.java")), Language::Java);
        assert_eq!(detect(&PathBuf::from("main.c")), Language::C);
        assert_eq!(detect(&PathBuf::from("main.h")), Language::C);
        assert_eq!(detect(&PathBuf::from("main.cpp")), Language::Cpp);
        assert_eq!(detect(&PathBuf::from("main.cc")), Language::Cpp);
        assert_eq!(detect(&PathBuf::from("main.hpp")), Language::Cpp);
    }

    #[test]
    fn detect_scripting_languages() {
        assert_eq!(detect(&PathBuf::from("app.rb")), Language::Ruby);
        assert_eq!(detect(&PathBuf::from("script.sh")), Language::Shell);
        assert_eq!(detect(&PathBuf::from("script.lua")), Language::Lua);
        assert_eq!(detect(&PathBuf::from("app.php")), Language::Php);
        assert_eq!(detect(&PathBuf::from("app.ex")), Language::Elixir);
    }

    #[test]
    fn detect_config_files() {
        assert_eq!(detect(&PathBuf::from("config.toml")), Language::Toml);
        assert_eq!(detect(&PathBuf::from("config.yml")), Language::Yaml);
        assert_eq!(detect(&PathBuf::from("config.yaml")), Language::Yaml);
        assert_eq!(detect(&PathBuf::from("data.json")), Language::Json);
        assert_eq!(detect(&PathBuf::from("README.md")), Language::Markdown);
    }

    #[test]
    fn detect_special_filenames() {
        assert_eq!(detect(&PathBuf::from("Makefile")), Language::Shell);
        assert_eq!(detect(&PathBuf::from("Dockerfile")), Language::Shell);
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect(&PathBuf::from("data.bin")), Language::Unknown);
        assert_eq!(detect(&PathBuf::from("file")), Language::Unknown);
        assert_eq!(detect(&PathBuf::from("image.png")), Language::Unknown);
    }

    #[test]
    fn is_source_code_excludes_config() {
        assert!(!Language::Toml.is_source_code());
        assert!(!Language::Yaml.is_source_code());
        assert!(!Language::Json.is_source_code());
        assert!(!Language::Markdown.is_source_code());
        assert!(!Language::Unknown.is_source_code());
    }

    #[test]
    fn is_source_code_includes_code() {
        assert!(Language::Python.is_source_code());
        assert!(Language::Rust.is_source_code());
        assert!(Language::TypeScript.is_source_code());
        assert!(Language::Go.is_source_code());
        assert!(Language::Css.is_source_code());
    }

    #[test]
    fn tree_sitter_support() {
        assert!(Language::Python.has_tree_sitter_support());
        assert!(Language::JavaScript.has_tree_sitter_support());
        assert!(Language::Rust.has_tree_sitter_support());
        assert!(Language::Go.has_tree_sitter_support());
        assert!(Language::Php.has_tree_sitter_support());
        assert!(!Language::Shell.has_tree_sitter_support());
        assert!(!Language::Unknown.has_tree_sitter_support());
    }
}
