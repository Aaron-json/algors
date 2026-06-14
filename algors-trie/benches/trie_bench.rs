mod common;

use common::{bench_inserts, bench_lookups};
use criterion::{Criterion, criterion_group, criterion_main};
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;

const COUNT: usize = 5_000;
const LEN: usize = 16;
const SEED: u64 = 42;

fn generate_random_keys(count: usize, len: usize) -> Vec<Vec<u8>> {
    let mut rng = StdRng::seed_from_u64(SEED);
    (0..count)
        .map(|_| {
            let mut key = vec![0u8; len];
            rng.fill_bytes(&mut key);
            key
        })
        .collect()
}

// benchmark insertion and lookup perf for seeded random inputs.
fn bench_random(c: &mut Criterion) {
    let keys = generate_random_keys(COUNT, LEN);
    bench_lookups(c, "Random-Lookup", &keys);
    bench_inserts(c, "Random-Insert", &keys);
}

criterion_group!(benches, bench_random);
criterion_main!(benches);
