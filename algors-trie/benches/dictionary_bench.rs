use algors_trie::TrieMap;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn load_dictionary() -> Vec<String> {
    let file = File::open("/usr/share/dict/words").expect("Could not open dictionary");
    let reader = BufReader::new(file);
    reader.lines().map_while(Result::ok).take(50_000).collect()
}

fn bench_dict_lookups(c: &mut Criterion) {
    let words = load_dictionary();
    let mut trie = TrieMap::new();
    let mut btree = BTreeMap::new();

    for word in &words {
        trie.insert(word, 0usize);
        btree.insert(word.clone(), 0usize);
    }

    let mut group = c.benchmark_group("DictLookup");
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
    let words = load_dictionary();

    let mut group = c.benchmark_group("DictInsert");
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

criterion_group!(benches, bench_dict_lookups, bench_dict_inserts);
criterion_main!(benches);
