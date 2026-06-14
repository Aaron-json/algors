mod common;

use common::{TrackingAllocator, bench_inserts, bench_lookups, compare_memory};
use criterion::{Criterion, criterion_group, criterion_main};
use std::fs;
use std::path::PathBuf;

#[global_allocator]
static A: TrackingAllocator = TrackingAllocator;

const LIMIT: usize = 500_000;

/// Loads files from the machine's home path to use as input into
/// the trie benches and/or tests.
fn scan_home_paths(limit: usize) -> Vec<String> {
    let mut paths = Vec::with_capacity(limit);
    let root = std::env::var("HOME")
        .map(PathBuf::from)
        .expect("HOME environment variable not set on the system");

    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                paths.push(path.to_string_lossy().into_owned());
                if paths.len() >= limit {
                    return paths;
                }
            }
        }
    }

    if paths.is_empty() {
        panic!("Could not find any files in home directory to benchmark.");
    }
    paths
}

fn bench_path(c: &mut Criterion) {
    let paths = scan_home_paths(LIMIT);

    compare_memory("Path", &paths);

    bench_lookups(c, "Path-Lookup", &paths);
    bench_inserts(c, "Path-Insert", &paths);
}

criterion_group!(benches, bench_path);
criterion_main!(benches);
