use algors_trie::TrieMap;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use std::collections::BTreeMap;

fn bench_lookups(c: &mut Criterion) {
    let mut trie = TrieMap::new();
    let mut btree = BTreeMap::new();
    let mut rng = StdRng::seed_from_u64(42);
    let mut keys = Vec::new();

    for _ in 0..1000 {
        let mut key = vec![0u8; 16];
        rng.fill_bytes(&mut key);
        trie.insert(&key, 0usize);
        btree.insert(key.clone(), 0usize);
        keys.push(key);
    }

    let mut group = c.benchmark_group("Lookup");
    group.bench_function("TrieMap", |b| {
        b.iter(|| {
            for key in &keys {
                black_box(trie.get(key));
            }
        })
    });
    group.bench_function("BTreeMap", |b| {
        b.iter(|| {
            for key in &keys {
                black_box(btree.get(key));
            }
        })
    });
    group.finish();
}

fn bench_inserts(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(42);
    let mut keys = Vec::new();
    for _ in 0..1000 {
        let mut key = vec![0u8; 16];
        rng.fill_bytes(&mut key);
        keys.push(key);
    }

    let mut group = c.benchmark_group("Insert");
    group.bench_function("TrieMap", |b| {
        b.iter(|| {
            let mut trie = TrieMap::new();
            for key in &keys {
                trie.insert(key, 0usize);
            }
        })
    });
    group.bench_function("BTreeMap", |b| {
        b.iter(|| {
            let mut btree = BTreeMap::new();
            for key in &keys {
                btree.insert(key.clone(), 0usize);
            }
        })
    });
    group.finish();
}

criterion_group!(benches, bench_lookups, bench_inserts);
criterion_main!(benches);
