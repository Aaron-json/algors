use algors_bloom::bloom::Bloom;
use algors_bloom::hash::{XXH_SECRET_SIZE_MIN, XxHasher};
use proptest::prelude::*;
use rand::{Rng, thread_rng};

/// Helper to create a XxHasher for testing.
fn test_hasher() -> XxHasher {
    let secret = vec![0x42u8; XXH_SECRET_SIZE_MIN].into_boxed_slice();
    XxHasher::new(secret)
}

#[test]
fn test_zero_false_negatives() {
    let mut bloom = Bloom::with_hints(1000, 0.01, test_hasher());
    let mut items = Vec::new();
    let mut rng = thread_rng();

    // insert 500 random items and chek that they are all found
    for _ in 0..500 {
        let mut item = [0u8; 32];
        rng.fill(&mut item);
        bloom.insert(&item);
        items.push(item);
    }

    for item in items {
        assert!(
            bloom.contains(&item),
            "Bloom filter returned false negative for item {:?}",
            item
        );
    }
}

#[test]
fn test_statistical_false_positive_rate() {
    let n = 100_000;
    let p_target = 0.01;
    let mut bloom = Bloom::with_hints(n, p_target, test_hasher());

    for i in 0..n {
        // fast way to generate unique strings
        bloom.insert(i.to_le_bytes());
    }

    // check items that were definitely not inserted
    let mut false_positives = 0;
    for i in n..2 * n {
        if bloom.contains(i.to_le_bytes()) {
            false_positives += 1;
        }
    }

    let actual_fpr = false_positives as f64 / n as f64;

    // Tightened bound: 1.2x of target.
    // With 100k samples, 3 standard deviations is roughly 1.1x,
    // so 1.2x is a very safe but much tighter bound.
    assert!(
        actual_fpr <= p_target * 1.2,
        "FPR too high: target {}, actual {} ({} hits)",
        p_target,
        actual_fpr,
        false_positives
    );

    // should also not be 0
    // (statistically impossible for 100k items at 1% FPR)
    assert!(
        false_positives > 0,
        "FPR suspiciously low (0 hits); check hashing distribution"
    );
}

#[test]
fn test_large_capacity_overflow_safety() {
    // test that very low FPRs (which lead to very large bitsets)
    // don't cause overflows in index calculation.
    let size_hint = 1_000_000;
    let p_false = 0.000001;
    let bloom = Bloom::with_hints(size_hint, p_false, test_hasher());

    let item = b"large_capacity_test";
    bloom.contains(item);

    // check that the bitset was actually sized up correctly
    assert!(bloom.bits_size() > size_hint as u64);
}

#[test]
fn test_hash_count_sanity() {
    // For n=1000, p=0.01, k should be approximately 7.
    let bloom = Bloom::with_hints(1000, 0.01, test_hasher());
    assert!(bloom.hash_count() >= 6 && bloom.hash_count() <= 8);
}

// Property-based testing for a wide range of sizes and FPRs
proptest! {
    #[test]
    fn prop_no_false_negatives(
        n in 100..1000usize,
        p in 0.001..0.1f64,
        data in prop::collection::vec(prop::collection::vec(0u8..255u8, 1..32), 10..50)
    ) {
        let mut bloom = Bloom::with_hints(n, p, test_hasher());
        for item in &data {
            bloom.insert(item);
        }
        for item in &data {
            prop_assert!(bloom.contains(item));
        }
    }
}
