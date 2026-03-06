use std::path::PathBuf;

use super::{ConfigKind, DetectedConfig};

struct ConfigPattern {
    names: &'static [&'static str],
    kind: ConfigKind,
}

const CONFIG_PATTERNS: &[ConfigPattern] = &[
    // Formatters
    ConfigPattern {
        names: &[
            ".prettierrc",
            ".prettierrc.json",
            ".prettierrc.yml",
            ".prettierrc.yaml",
            ".prettierrc.js",
            ".prettierrc.cjs",
            "prettier.config.js",
            "prettier.config.cjs",
            "rustfmt.toml",
            ".rustfmt.toml",
            ".editorconfig",
            ".clang-format",
            ".scalafmt.conf",
            "biome.json",
        ],
        kind: ConfigKind::Formatter,
    },
    // Linters
    ConfigPattern {
        names: &[
            ".eslintrc",
            ".eslintrc.json",
            ".eslintrc.yml",
            ".eslintrc.js",
            ".eslintrc.cjs",
            "eslint.config.js",
            "eslint.config.mjs",
            ".pylintrc",
            ".flake8",
            "ruff.toml",
            ".clippy.toml",
            ".rubocop.yml",
            ".golangci.yml",
            ".golangci.yaml",
            "biome.json",
        ],
        kind: ConfigKind::Linter,
    },
    // Type checkers
    ConfigPattern {
        names: &[
            "tsconfig.json",
            "jsconfig.json",
            "mypy.ini",
            ".mypy.ini",
            "pyrightconfig.json",
        ],
        kind: ConfigKind::TypeChecker,
    },
    // Test frameworks
    ConfigPattern {
        names: &[
            "jest.config.js",
            "jest.config.ts",
            "jest.config.mjs",
            "vitest.config.js",
            "vitest.config.ts",
            "vitest.config.mjs",
            "pytest.ini",
            "setup.cfg",
            "conftest.py",
            "phpunit.xml",
            "phpunit.xml.dist",
            ".rspec",
        ],
        kind: ConfigKind::TestFramework,
    },
    // CI
    ConfigPattern {
        names: &[
            ".github",
            ".gitlab-ci.yml",
            ".travis.yml",
            "Jenkinsfile",
            ".circleci",
            "azure-pipelines.yml",
            ".buildkite",
            "bitbucket-pipelines.yml",
        ],
        kind: ConfigKind::CI,
    },
    // Docker
    ConfigPattern {
        names: &[
            "Dockerfile",
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ],
        kind: ConfigKind::Docker,
    },
    // Dependency managers
    ConfigPattern {
        names: &[
            "package.json",
            "Cargo.toml",
            "go.mod",
            "requirements.txt",
            "pyproject.toml",
            "setup.py",
            "Pipfile",
            "Gemfile",
            "build.gradle",
            "build.gradle.kts",
            "pom.xml",
            "composer.json",
            "Package.swift",
            "pubspec.yaml",
            "mix.exs",
        ],
        kind: ConfigKind::DependencyManager,
    },
    // Lockfiles
    ConfigPattern {
        names: &[
            "package-lock.json",
            "yarn.lock",
            "pnpm-lock.yaml",
            "Cargo.lock",
            "go.sum",
            "Pipfile.lock",
            "poetry.lock",
            "Gemfile.lock",
            "composer.lock",
        ],
        kind: ConfigKind::Lockfile,
    },
    // Build scripts
    ConfigPattern {
        names: &[
            "Makefile",
            "CMakeLists.txt",
            "build.rs",
            "Rakefile",
            "Taskfile.yml",
            "justfile",
        ],
        kind: ConfigKind::BuildScript,
    },
    // Editor config
    ConfigPattern {
        names: &[".editorconfig"],
        kind: ConfigKind::EditorConfig,
    },
    // Documentation
    ConfigPattern {
        names: &[
            "README.md",
            "README.rst",
            "README.txt",
            "README",
            "readme.md",
        ],
        kind: ConfigKind::Readme,
    },
    ConfigPattern {
        names: &["ARCHITECTURE.md", "DESIGN.md", "docs", "doc", "AGENTS.md"],
        kind: ConfigKind::ArchitectureDocs,
    },
    ConfigPattern {
        names: &["CONTRIBUTING.md", "CONTRIBUTING.rst", "CONTRIBUTING"],
        kind: ConfigKind::Contributing,
    },
    // Git
    ConfigPattern {
        names: &[".gitignore"],
        kind: ConfigKind::GitIgnore,
    },
];

pub fn detect_configs(paths: &[PathBuf]) -> Vec<DetectedConfig> {
    let mut found = Vec::new();

    for base_path in paths {
        let canonical = base_path
            .canonicalize()
            .unwrap_or_else(|_| base_path.clone());
        scan_dir_for_configs(&canonical, &mut found, 0);
    }

    found
}

fn scan_dir_for_configs(dir: &std::path::Path, found: &mut Vec<DetectedConfig>, depth: usize) {
    if depth > 3 {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        for pattern in CONFIG_PATTERNS {
            if pattern.names.contains(&name.as_str()) {
                found.push(DetectedConfig {
                    kind: pattern.kind.clone(),
                    path: path.clone(),
                });
            }
        }

        if path.is_dir()
            && !name.starts_with('.')
            && name != "node_modules"
            && name != "target"
            && name != "vendor"
        {
            scan_dir_for_configs(&path, found, depth + 1);
        }
    }

    check_pyproject_toml(dir, found);
}

fn check_pyproject_toml(dir: &std::path::Path, found: &mut Vec<DetectedConfig>) {
    let pyproject = dir.join("pyproject.toml");
    if !pyproject.exists() {
        return;
    }
    let content = match std::fs::read_to_string(&pyproject) {
        Ok(c) => c,
        Err(_) => return,
    };

    if content.contains("[tool.ruff]") || content.contains("[tool.pylint]") {
        found.push(DetectedConfig {
            kind: ConfigKind::Linter,
            path: pyproject.clone(),
        });
    }
    if content.contains("[tool.mypy]") || content.contains("[tool.pyright]") {
        found.push(DetectedConfig {
            kind: ConfigKind::TypeChecker,
            path: pyproject.clone(),
        });
    }
    if content.contains("[tool.pytest") {
        found.push(DetectedConfig {
            kind: ConfigKind::TestFramework,
            path: pyproject.clone(),
        });
    }
    if content.contains("[tool.black]") || content.contains("[tool.isort]") {
        found.push(DetectedConfig {
            kind: ConfigKind::Formatter,
            path: pyproject.clone(),
        });
    }
}
