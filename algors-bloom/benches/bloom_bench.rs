use algors_bloom::bloom::Bloom;
use algors_bloom::hash::{XXH_SECRET_SIZE_MIN, XxHasher};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rand::{Rng, thread_rng};

fn bench_bloom(c: &mut Criterion) {
    let n = 100_000_000;
    let p = 0.01;
    let secret = vec![0x42u8; XXH_SECRET_SIZE_MIN].into_boxed_slice();

    let mut bloom = Bloom::with_hints(n, p, XxHasher::new(secret));

    let mut rng = thread_rng();
    let num_items = 1_000_000;
    let mut items = Vec::with_capacity(num_items);
    for _ in 0..num_items {
        let mut item = [0u8; 32];
        rng.fill(&mut item);
        items.push(item);
    }

    // Benchmark Inserts
    let mut group = c.benchmark_group("Bloom_Insert_1M_Batch");
    group.sample_size(10);
    group.bench_function("Current_Implementation", |b| {
        b.iter(|| {
            for item in items.iter() {
                bloom.insert(black_box(item));
            }
        })
    });
    group.finish();

    // Benchmark Contains
    let mut group = c.benchmark_group("Bloom_Contains_1M_Batch");
    group.sample_size(10);
    group.bench_function("Current_Implementation", |b| {
        b.iter(|| {
            for item in items.iter() {
                black_box(bloom.contains(black_box(item)));
            }
        })
    });
    group.finish();
}

criterion_group!(benches, bench_bloom);
criterion_main!(benches);
