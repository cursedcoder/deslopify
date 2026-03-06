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
