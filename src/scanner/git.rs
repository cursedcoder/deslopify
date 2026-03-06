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
    pub window_days: u32,
    pub window_label: String,
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
            window_days: 180,
            window_label: "6 months".to_string(),
        }
    }
}

pub fn analyze(
    repo_path: &Path,
    files: &[FileEntry],
    months_override: Option<u32>,
) -> Option<GitActivity> {
    if !repo_path.join(".git").exists() && !has_git_dir(repo_path) {
        return None;
    }

    let window_days = match months_override {
        Some(m) => m * 30,
        None => match repo_age_days(repo_path) {
            Some(age) => auto_window_days(age),
            None => 180,
        },
    };

    let file_commits = git_file_frequency_days(repo_path, window_days)?;
    let total_commits = count_commits_days(repo_path, window_days).unwrap_or(0);

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
        window_days,
        window_label: format_window(window_days),
    })
}

fn has_git_dir(path: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output();
    matches!(output, Ok(o) if o.status.success())
}

/// Get the repo age in days from the first (root) commit.
fn repo_age_days(repo_path: &Path) -> Option<u32> {
    let root = Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !root.status.success() {
        return None;
    }

    let root_hash = String::from_utf8_lossy(&root.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();

    let ts_output = Command::new("git")
        .args(["log", "-1", "--format=%at", &root_hash])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !ts_output.status.success() {
        return None;
    }

    let timestamp: u64 = String::from_utf8_lossy(&ts_output.stdout)
        .trim()
        .parse()
        .ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let age_secs = now.saturating_sub(timestamp);
    Some((age_secs / 86400) as u32)
}

/// Scale the lookback window to ~1/3 of repo age with a 7-day floor.
/// Young repos get a tight window so scaffolding separates from active code.
/// Old repos naturally get longer windows — a file untouched for 1/3 of a
/// project's life is genuinely frozen.
pub fn auto_window_days(repo_age: u32) -> u32 {
    (repo_age / 3).max(7)
}

fn format_window(days: u32) -> String {
    if days < 14 {
        format!("{} days", days)
    } else if days < 60 {
        format!("{} weeks", days / 7)
    } else {
        format!("{} months", days / 30)
    }
}

fn git_file_frequency_days(repo_path: &Path, days: u32) -> Option<HashMap<PathBuf, usize>> {
    let since = format!("{} days ago", days);
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

fn count_commits_days(repo_path: &Path, days: u32) -> Option<usize> {
    let output = Command::new("git")
        .args([
            "rev-list",
            "--count",
            "--since",
            &format!("{} days ago", days),
            "HEAD",
        ])
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
    fn auto_window_young_repo() {
        // 2-week-old repo (14 days) → window = 14/3 = 4, clamped to min 7
        assert_eq!(auto_window_days(14), 7);
    }

    #[test]
    fn auto_window_one_month_repo() {
        assert_eq!(auto_window_days(30), 10);
    }

    #[test]
    fn auto_window_three_month_repo() {
        assert_eq!(auto_window_days(90), 30);
    }

    #[test]
    fn auto_window_one_year_repo() {
        // 365/3 = 121 days (~4 months)
        assert_eq!(auto_window_days(365), 121);
    }

    #[test]
    fn auto_window_old_repo_no_cap() {
        // 2-year repo (730 days) → 730/3 = 243 days (~8 months), no cap
        assert_eq!(auto_window_days(730), 243);
    }

    #[test]
    fn auto_window_very_old_repo() {
        // 5-year repo (1825 days) → 608 days (~20 months)
        assert_eq!(auto_window_days(1825), 608);
    }

    #[test]
    fn format_window_days() {
        assert_eq!(format_window(7), "7 days");
        assert_eq!(format_window(13), "13 days");
    }

    #[test]
    fn format_window_weeks() {
        assert_eq!(format_window(14), "2 weeks");
        assert_eq!(format_window(30), "4 weeks");
        assert_eq!(format_window(56), "8 weeks");
    }

    #[test]
    fn format_window_months() {
        assert_eq!(format_window(60), "2 months");
        assert_eq!(format_window(180), "6 months");
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
