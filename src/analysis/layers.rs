use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::ImportInfo;
use crate::scanner::ScanResult;

#[derive(Debug)]
pub struct LayerAnalysis {
    pub group_count: usize,
    pub bidirectional_pairs: Vec<(String, String)>,
    pub god_modules: Vec<String>,
    pub layering_score: f64,
}

/// Analyze import graph for directional layering between top-level directory groups.
pub fn analyze_layers(imports: &[ImportInfo], scan: &ScanResult) -> LayerAnalysis {
    let groups = build_group_map(scan);
    if groups.is_empty() {
        return LayerAnalysis {
            group_count: 0,
            bidirectional_pairs: vec![],
            god_modules: vec![],
            layering_score: 1.0,
        };
    }

    let group_edges = build_group_edges(imports, &groups);
    let group_count = groups.values().collect::<HashSet<_>>().len();
    let bidirectional_pairs = find_bidirectional_deps(&group_edges);
    let god_modules = find_god_modules(&group_edges, group_count);
    let layering_score = compute_layering_score(&group_edges);

    LayerAnalysis {
        group_count,
        bidirectional_pairs,
        god_modules,
        layering_score,
    }
}

/// Map each file path to its top-level directory group.
fn build_group_map(scan: &ScanResult) -> HashMap<String, String> {
    let mut groups = HashMap::new();
    for file in &scan.files {
        if let Some(group) = extract_group(&file.path) {
            groups.insert(file.path.to_string_lossy().to_string(), group);
        }
    }
    groups
}

fn extract_group(path: &Path) -> Option<String> {
    let components: Vec<_> = path.components().collect();
    if components.len() < 2 {
        return None;
    }
    // Use first two meaningful directory components as the group
    // e.g., src/api/routes.py -> "src/api", lib/utils/math.rb -> "lib/utils"
    let mut parts = Vec::new();
    for comp in &components {
        let s = comp.as_os_str().to_string_lossy().to_string();
        if s == "." || s == ".." {
            continue;
        }
        parts.push(s);
        if parts.len() == 2 {
            break;
        }
    }
    if parts.len() >= 2 {
        // Only use first directory for grouping (the second may be a file)
        if components.len() > 2 {
            Some(format!("{}/{}", parts[0], parts[1]))
        } else {
            Some(parts[0].clone())
        }
    } else {
        Some(parts.join("/"))
    }
}

/// Pre-built index for fast import-to-group resolution.
struct GroupIndex {
    component_to_groups: HashMap<String, HashSet<String>>,
    all_groups: HashSet<String>,
}

impl GroupIndex {
    fn build(groups: &HashMap<String, String>) -> Self {
        let mut component_to_groups: HashMap<String, HashSet<String>> = HashMap::new();
        let mut all_groups = HashSet::new();

        for group in groups.values() {
            all_groups.insert(group.clone());
            for component in group.split('/') {
                component_to_groups
                    .entry(component.to_string())
                    .or_default()
                    .insert(group.clone());
            }
        }

        Self {
            component_to_groups,
            all_groups,
        }
    }

    fn resolve(&self, module_path: &str) -> Option<&String> {
        // Try exact group match first
        if self.all_groups.contains(module_path) {
            return self.all_groups.get(module_path);
        }

        // Match by first meaningful path component of the import
        for component in module_path.split(['/', '.', ':', '\\']) {
            if component.is_empty() {
                continue;
            }
            if let Some(matching_groups) = self.component_to_groups.get(component) {
                return matching_groups.iter().next();
            }
        }
        None
    }
}

/// Build directed edges between groups based on imports.
fn build_group_edges(
    imports: &[ImportInfo],
    groups: &HashMap<String, String>,
) -> HashMap<String, HashSet<String>> {
    let index = GroupIndex::build(groups);
    let mut edges: HashMap<String, HashSet<String>> = HashMap::new();

    for imp in imports {
        let from_file = imp.file.to_string_lossy().to_string();
        let from_group = match groups.get(&from_file) {
            Some(g) => g.clone(),
            None => continue,
        };

        if let Some(to_group) = index.resolve(&imp.module_path) {
            if from_group != *to_group {
                edges
                    .entry(from_group)
                    .or_default()
                    .insert(to_group.clone());
            }
        }
    }
    edges
}

fn find_bidirectional_deps(edges: &HashMap<String, HashSet<String>>) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    for (from, targets) in edges {
        for to in targets {
            if let Some(back_targets) = edges.get(to) {
                if back_targets.contains(from) {
                    let pair = if from < to {
                        (from.clone(), to.clone())
                    } else {
                        (to.clone(), from.clone())
                    };
                    if seen.insert(pair.clone()) {
                        pairs.push(pair);
                    }
                }
            }
        }
    }
    pairs
}

fn find_god_modules(edges: &HashMap<String, HashSet<String>>, total_groups: usize) -> Vec<String> {
    if total_groups < 3 {
        return vec![];
    }

    let mut incoming: HashMap<String, usize> = HashMap::new();
    for targets in edges.values() {
        for target in targets {
            *incoming.entry(target.clone()).or_insert(0) += 1;
        }
    }

    let threshold = (total_groups as f64 * 0.6).ceil() as usize;
    incoming
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(module, _)| module)
        .collect()
}

fn compute_layering_score(edges: &HashMap<String, HashSet<String>>) -> f64 {
    let mut total_edges = 0usize;
    let mut unidirectional = 0usize;

    let mut counted: HashSet<(String, String)> = HashSet::new();
    for (from, targets) in edges {
        for to in targets {
            let pair = if from < to {
                (from.clone(), to.clone())
            } else {
                (to.clone(), from.clone())
            };
            if !counted.insert(pair) {
                continue;
            }
            total_edges += 1;
            let reverse_exists = edges.get(to).map(|t| t.contains(from)).unwrap_or(false);
            if !reverse_exists {
                unidirectional += 1;
            }
        }
    }

    if total_edges == 0 {
        1.0
    } else {
        unidirectional as f64 / total_edges as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn make_imports(pairs: &[(&str, &str)]) -> Vec<ImportInfo> {
        pairs
            .iter()
            .map(|(from, to)| ImportInfo {
                file: std::path::PathBuf::from(from),
                module_path: to.to_string(),
                line: 1,
            })
            .collect()
    }

    #[test]
    fn no_imports_clean_layering() {
        let imports: Vec<ImportInfo> = vec![];
        let scan = ScanResult {
            files: vec![],
            language_breakdown: vec![],
            configs: vec![],
            quality: crate::scanner::quality::QualitySignals::default(),
            git_activity: None,
            total_files: 0,
            total_lines: 0,
            total_bytes: 0,
            test_file_count: 0,
            source_file_count: 0,
            max_file_lines: 0,
            avg_file_lines: 0,
            max_dir_depth: 0,
            top_level_dirs: 0,
        };
        let result = analyze_layers(&imports, &scan);
        assert_eq!(result.layering_score, 1.0);
        assert!(result.bidirectional_pairs.is_empty());
    }

    #[test]
    fn bidirectional_detected() {
        let mut edges: HashMap<String, HashSet<String>> = HashMap::new();
        edges
            .entry("src/api".to_string())
            .or_default()
            .insert("src/db".to_string());
        edges
            .entry("src/db".to_string())
            .or_default()
            .insert("src/api".to_string());

        let pairs = find_bidirectional_deps(&edges);
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn unidirectional_no_violations() {
        let mut edges: HashMap<String, HashSet<String>> = HashMap::new();
        edges
            .entry("src/api".to_string())
            .or_default()
            .insert("src/db".to_string());
        edges
            .entry("src/api".to_string())
            .or_default()
            .insert("src/models".to_string());

        let pairs = find_bidirectional_deps(&edges);
        assert!(pairs.is_empty());
        assert_eq!(compute_layering_score(&edges), 1.0);
    }

    #[test]
    fn god_module_detected() {
        let mut edges: HashMap<String, HashSet<String>> = HashMap::new();
        for from in &["src/api", "src/web", "src/cli", "src/workers"] {
            edges
                .entry(from.to_string())
                .or_default()
                .insert("src/core".to_string());
        }

        let gods = find_god_modules(&edges, 5);
        assert!(
            gods.contains(&"src/core".to_string()),
            "src/core should be detected as a god module"
        );
    }

    #[test]
    fn layering_score_mixed() {
        let mut edges: HashMap<String, HashSet<String>> = HashMap::new();
        edges
            .entry("a".to_string())
            .or_default()
            .insert("b".to_string());
        edges
            .entry("b".to_string())
            .or_default()
            .insert("a".to_string());
        edges
            .entry("a".to_string())
            .or_default()
            .insert("c".to_string());

        let score = compute_layering_score(&edges);
        assert!(
            score > 0.4 && score < 0.6,
            "1 unidirectional out of 2 pairs = 0.5, got {:.2}",
            score
        );
    }

    #[test]
    fn extract_group_nested() {
        let group = extract_group(Path::new("src/api/routes.py"));
        assert_eq!(group, Some("src/api".to_string()));
    }

    #[test]
    fn extract_group_shallow() {
        let group = extract_group(Path::new("src/main.rs"));
        assert_eq!(group, Some("src".to_string()));
    }
}
