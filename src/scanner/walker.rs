use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub fn walk(paths: &[PathBuf], ignore_patterns: &[String]) -> Vec<PathBuf> {
    let mut results = Vec::new();

    for base_path in paths {
        let mut builder = WalkBuilder::new(base_path);
        builder.hidden(true).git_ignore(true).git_global(true);

        for pattern in ignore_patterns {
            let _ = builder.add_ignore(Path::new(pattern));
        }

        for entry in builder.build().flatten() {
            let path = entry.path();
            if path.is_file() {
                if should_skip(path, ignore_patterns) {
                    continue;
                }
                results.push(path.to_path_buf());
            }
        }
    }

    results
}

fn should_skip(path: &Path, ignore_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in ignore_patterns {
        if path_str.contains(pattern.trim_matches('*').trim_matches('/')) {
            return true;
        }
    }

    let skip_dirs = [
        "node_modules",
        ".git",
        "__pycache__",
        ".venv",
        "venv",
        "target",
        "dist",
        "build",
        ".next",
        ".nuxt",
        "vendor",
        ".tox",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        "coverage",
        ".coverage",
        ".idea",
        ".vscode",
        ".cursor",
    ];

    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if skip_dirs.contains(&name.as_ref()) {
            return true;
        }
    }

    false
}
