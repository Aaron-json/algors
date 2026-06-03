use algors_trie::TrieMap;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

const TARGET_SIZE: usize = 1_000_000;

/// Returns an iterator that yields up to 1,000,000 keys based on the system dictionary.
fn load_million_keys() -> impl Iterator<Item = String> {
    let file = File::open("/usr/share/dict/words").expect("Could not open dictionary");
    let base = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .take(TARGET_SIZE);

    base
}

fn bench_dict_lookups(c: &mut Criterion) {
    let words: Vec<String> = load_million_keys().collect();
    let mut trie = TrieMap::new();
    let mut btree = BTreeMap::new();

    for word in &words {
        trie.insert(word, 0usize);
        btree.insert(word.clone(), 0usize);
    }

    let mut group = c.benchmark_group("DictLookup");
    group.sample_size(10);

    group.bench_function("TrieMap", |b| {
        b.iter(|| {
            for word in &words {
                black_box(trie.get(word));
            }
        })
    });

    group.bench_function("BTreeMap", |b| {
        b.iter(|| {
            for word in &words {
                black_box(btree.get(word));
            }
        })
    });

    group.finish();
}

fn bench_dict_inserts(c: &mut Criterion) {
    let words: Vec<String> = load_million_keys().collect();

    let mut group = c.benchmark_group("DictInsert");
    group.sample_size(10);

    group.bench_function("TrieMap", |b| {
        b.iter(|| {
            let mut trie = TrieMap::new();
            for word in &words {
                trie.insert(word, 0usize);
            }
        })
    });

    group.bench_function("BTreeMap", |b| {
        b.iter(|| {
            let mut btree = BTreeMap::new();
            for word in &words {
                btree.insert(word.clone(), 0usize);
            }
        })
    });

    group.finish();
}

fn bench_dict_removes(c: &mut Criterion) {
    let words: Vec<String> = load_million_keys().collect();

    let mut group = c.benchmark_group("DictRemove");
    group.sample_size(10);

    group.bench_function("TrieMap", |b| {
        b.iter_batched(
            || {
                let mut trie = TrieMap::new();
                for word in &words {
                    trie.insert(word, 0usize);
                }
                trie
            },
            |mut trie| {
                for word in &words {
                    black_box(trie.remove(word));
                }
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.bench_function("BTreeMap", |b| {
        b.iter_batched(
            || {
                let mut btree = BTreeMap::new();
                for word in &words {
                    btree.insert(word.clone(), 0usize);
                }
                btree
            },
            |mut btree| {
                for word in &words {
                    black_box(btree.remove(word));
                }
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_dict_lookups,
    bench_dict_inserts,
    bench_dict_removes
);
criterion_main!(benches);
