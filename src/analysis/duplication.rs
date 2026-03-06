use std::collections::HashMap;

use crate::scanner::FileEntry;

use super::DuplicateCluster;

const MIN_CHUNK_LINES: usize = 6;

pub fn find_duplicates(files: &[&FileEntry]) -> Vec<DuplicateCluster> {
    let mut chunk_map: HashMap<u64, Vec<(std::path::PathBuf, usize)>> = HashMap::new();

    for file in files {
        let lines: Vec<&str> = file.content.lines().collect();
        if lines.len() < MIN_CHUNK_LINES {
            continue;
        }

        for start in 0..lines.len().saturating_sub(MIN_CHUNK_LINES) {
            let chunk: String = lines[start..start + MIN_CHUNK_LINES]
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");

            if chunk.len() < 40 {
                continue;
            }

            let hash = simple_hash(&chunk);
            chunk_map
                .entry(hash)
                .or_default()
                .push((file.path.clone(), start + 1));
        }
    }

    chunk_map
        .into_iter()
        .filter(|(_, locations)| {
            if locations.len() < 2 {
                return false;
            }
            // Deduplicate: must span at least 2 distinct files or 2 distant locations
            let unique_files: std::collections::HashSet<_> =
                locations.iter().map(|(p, _)| p).collect();
            unique_files.len() >= 2
        })
        .map(|(hash, locations)| DuplicateCluster {
            hash,
            locations,
            line_count: MIN_CHUNK_LINES,
        })
        .collect()
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}
