use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::FileEntry;

#[derive(Debug)]
pub struct HotFile {
    pub path: PathBuf,
    pub commit_count: usize,
    pub lines: usize,
}

#[derive(Debug)]
pub struct GitActivity {
    pub is_git_repo: bool,
    pub active_files: usize,
    pub active_lines: usize,
    pub active_bytes: u64,
    pub frozen_files: usize,
    pub frozen_bytes: u64,
    pub hot_files: Vec<HotFile>,
    pub total_commits: usize,
    pub window_months: u32,
}

impl Default for GitActivity {
    fn default() -> Self {
        Self {
            is_git_repo: false,
            active_files: 0,
            active_lines: 0,
            active_bytes: 0,
            frozen_files: 0,
            frozen_bytes: 0,
            hot_files: Vec::new(),
            total_commits: 0,
            window_months: 6,
        }
    }
}

pub fn analyze(repo_path: &Path, files: &[FileEntry], window_months: u32) -> Option<GitActivity> {
    if !repo_path.join(".git").exists() && !has_git_dir(repo_path) {
        return None;
    }

    let file_commits = git_file_frequency(repo_path, window_months)?;

    let total_commits = count_commits(repo_path, window_months).unwrap_or(0);

    let mut active_files = 0usize;
    let mut active_lines = 0usize;
    let mut active_bytes = 0u64;
    let mut frozen_files = 0usize;
    let mut frozen_bytes = 0u64;
    let mut hot: Vec<HotFile> = Vec::new();

    for file in files {
        let relative = relativize(repo_path, &file.path);
        if let Some(&count) = relative
            .as_ref()
            .and_then(|r| file_commits.get(r.as_path()))
        {
            active_files += 1;
            active_lines += file.line_count;
            active_bytes += file.size_bytes;
            hot.push(HotFile {
                path: file.path.clone(),
                commit_count: count,
                lines: file.line_count,
            });
        } else {
            frozen_files += 1;
            frozen_bytes += file.size_bytes;
        }
    }

    hot.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
    hot.truncate(10);

    Some(GitActivity {
        is_git_repo: true,
        active_files,
        active_lines,
        active_bytes,
        frozen_files,
        frozen_bytes,
        hot_files: hot,
        total_commits,
        window_months,
    })
}

fn has_git_dir(path: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

/// Run `git log` to get per-file commit counts within the time window.
fn git_file_frequency(repo_path: &Path, months: u32) -> Option<HashMap<PathBuf, usize>> {
    let since = format!("{} months ago", months);
    let output = Command::new("git")
        .args([
            "log",
            "--since",
            &since,
            "--format=",
            "--name-only",
            "--diff-filter=ACMR",
        ])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut counts: HashMap<PathBuf, usize> = HashMap::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        *counts.entry(PathBuf::from(trimmed)).or_insert(0) += 1;
    }

    Some(counts)
}

fn count_commits(repo_path: &Path, months: u32) -> Option<usize> {
    let since = format!("{} months ago", months);
    let output = Command::new("git")
        .args(["rev-list", "--count", "--since", &since, "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// Try to make a file path relative to the repo root.
fn relativize(repo_path: &Path, file_path: &Path) -> Option<PathBuf> {
    let repo_canonical = repo_path.canonicalize().ok()?;
    let file_canonical = file_path.canonicalize().ok()?;
    file_canonical
        .strip_prefix(&repo_canonical)
        .ok()
        .map(PathBuf::from)
}

/// Parse raw `git log --name-only` output into per-file commit counts.
/// Exposed for unit testing without requiring a real git repo.
pub fn parse_file_frequency(git_log_output: &str) -> HashMap<PathBuf, usize> {
    let mut counts: HashMap<PathBuf, usize> = HashMap::new();
    for line in git_log_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        *counts.entry(PathBuf::from(trimmed)).or_insert(0) += 1;
    }
    counts
}

/// Cross-reference parsed git frequency data with scanned files, returning
/// (active_bytes, active_lines, active_files, frozen_files, frozen_bytes, hot_files).
/// Exposed for unit testing.
pub fn cross_reference(
    file_commits: &HashMap<PathBuf, usize>,
    files: &[FileEntry],
) -> (u64, usize, usize, usize, u64, Vec<HotFile>) {
    let mut active_bytes = 0u64;
    let mut active_lines = 0usize;
    let mut active_files = 0usize;
    let mut frozen_files = 0usize;
    let mut frozen_bytes = 0u64;
    let mut hot: Vec<HotFile> = Vec::new();

    for file in files {
        if let Some(&count) = file_commits.get(&file.path) {
            active_files += 1;
            active_lines += file.line_count;
            active_bytes += file.size_bytes;
            hot.push(HotFile {
                path: file.path.clone(),
                commit_count: count,
                lines: file.line_count,
            });
        } else {
            frozen_files += 1;
            frozen_bytes += file.size_bytes;
        }
    }

    hot.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
    hot.truncate(10);

    (
        active_bytes,
        active_lines,
        active_files,
        frozen_files,
        frozen_bytes,
        hot,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::language::Language;

    fn make_file(path: &str, lines: usize, bytes: u64) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            language: Language::Rust,
            size_bytes: bytes,
            line_count: lines,
            content: String::new(),
        }
    }

    #[test]
    fn parse_git_log_output() {
        let log = "\
src/main.rs

src/lib.rs
src/main.rs

src/utils.rs
src/main.rs
";
        let freq = parse_file_frequency(log);
        assert_eq!(freq.get(Path::new("src/main.rs")), Some(&3));
        assert_eq!(freq.get(Path::new("src/lib.rs")), Some(&1));
        assert_eq!(freq.get(Path::new("src/utils.rs")), Some(&1));
        assert_eq!(freq.len(), 3);
    }

    #[test]
    fn parse_empty_log() {
        let freq = parse_file_frequency("");
        assert!(freq.is_empty());
    }

    #[test]
    fn cross_reference_splits_active_frozen() {
        let mut commits = HashMap::new();
        commits.insert(PathBuf::from("src/main.rs"), 5);
        commits.insert(PathBuf::from("src/lib.rs"), 2);

        let files = vec![
            make_file("src/main.rs", 100, 3000),
            make_file("src/lib.rs", 50, 1500),
            make_file("src/frozen.rs", 200, 6000),
            make_file("src/old.rs", 80, 2400),
        ];

        let (active_bytes, active_lines, active_count, frozen_count, frozen_bytes, hot) =
            cross_reference(&commits, &files);

        assert_eq!(active_count, 2);
        assert_eq!(active_lines, 150);
        assert_eq!(active_bytes, 4500);
        assert_eq!(frozen_count, 2);
        assert_eq!(frozen_bytes, 8400);
        assert_eq!(hot[0].path, PathBuf::from("src/main.rs"));
        assert_eq!(hot[0].commit_count, 5);
    }

    #[test]
    fn cross_reference_all_active() {
        let mut commits = HashMap::new();
        commits.insert(PathBuf::from("a.rs"), 1);
        commits.insert(PathBuf::from("b.rs"), 1);

        let files = vec![make_file("a.rs", 10, 300), make_file("b.rs", 20, 600)];

        let (active_bytes, _active_lines, active_count, frozen_count, frozen_bytes, _) =
            cross_reference(&commits, &files);

        assert_eq!(active_count, 2);
        assert_eq!(active_bytes, 900);
        assert_eq!(frozen_count, 0);
        assert_eq!(frozen_bytes, 0);
    }

    #[test]
    fn cross_reference_all_frozen() {
        let commits: HashMap<PathBuf, usize> = HashMap::new();

        let files = vec![make_file("a.rs", 10, 300)];

        let (active_bytes, _active_lines, active_count, frozen_count, _frozen_bytes, hot) =
            cross_reference(&commits, &files);

        assert_eq!(active_count, 0);
        assert_eq!(active_bytes, 0);
        assert_eq!(frozen_count, 1);
        assert!(hot.is_empty());
    }

    #[test]
    fn hot_files_sorted_and_truncated() {
        let mut commits = HashMap::new();
        for i in 0..15 {
            commits.insert(PathBuf::from(format!("f{}.rs", i)), i + 1);
        }

        let files: Vec<FileEntry> = (0..15)
            .map(|i| make_file(&format!("f{}.rs", i), 10, 100))
            .collect();

        let (_, _, _, _, _, hot) = cross_reference(&commits, &files);

        assert_eq!(hot.len(), 10);
        assert_eq!(hot[0].commit_count, 15);
        assert_eq!(hot[9].commit_count, 6);
    }
}
