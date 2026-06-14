mod common;

use common::{TrackingAllocator, bench_inserts, bench_lookups, bench_removes, compare_memory};
use criterion::{Criterion, criterion_group, criterion_main};
use std::fs::File;
use std::io::{BufRead, BufReader};

#[global_allocator]
static A: TrackingAllocator = TrackingAllocator;

const LIMIT: usize = 100_000;

fn load_system_dictionary(limit: usize) -> Vec<String> {
    let paths = [
        "/usr/share/dict/words",
        "/usr/share/dict/web2",
        "/usr/dict/words",
    ];
    for path in paths {
        if let Ok(file) = File::open(path) {
            return BufReader::new(file)
                .lines()
                .map_while(Result::ok)
                .filter(|l| !l.is_empty())
                .take(limit)
                .collect();
        }
    }
    panic!("No system dictionary found at common paths.");
}

fn bench_dict(c: &mut Criterion) {
    let words = load_system_dictionary(LIMIT);

    // mem
    compare_memory("Dictionary", &words);

    // perf
    bench_lookups(c, "Dictionary/Lookup", &words);
    bench_inserts(c, "Dictionary/Insert", &words);
    bench_removes(c, "Dictionary/Remove", &words);
}

criterion_group!(benches, bench_dict);
criterion_main!(benches);
