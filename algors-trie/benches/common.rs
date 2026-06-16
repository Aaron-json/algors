use algors_trie::TrieMap;
use criterion::{BatchSize, Criterion, black_box};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Wrapper allocator used to track allocation sized and memory used
pub struct TrackingAllocator;

pub static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Compares the memory usage against BtreeMap for the given keys.
/// Mostly useful for testing how much memory is saved by reusing the prefixes
pub fn compare_memory<K>(group_name: &str, keys: &[K])
where
    K: AsRef<[u8]> + Clone + Ord,
{
    let count = keys.len();
    if count == 0 {
        return;
    }

    let baseline = ALLOCATED.load(Ordering::Relaxed);

    let btree_used = {
        let mut map = BTreeMap::new();
        for key in keys {
            map.insert(key.clone(), 0u64);
        }
        ALLOCATED.load(Ordering::Relaxed) - baseline
    };

    let trie_used = {
        let mut trie = TrieMap::new();
        for key in keys {
            trie.insert(key, 0u64);
        }
        ALLOCATED.load(Ordering::Relaxed) - baseline
    };

    println!("\n--- Memory Comparison: {} ---", group_name);
    println!("Keys: {}", count);
    println!(
        "BTreeMap: {:.2} MB (avg {} bytes/key)",
        btree_used as f64 / 1_000_000.0,
        btree_used / count
    );
    println!(
        "TrieMap:  {:.2} MB (avg {} bytes/key)",
        trie_used as f64 / 1_000_000.0,
        trie_used / count
    );
    println!(
        "Improvement: {:.1}%\n",
        (1.0 - (trie_used as f64 / btree_used as f64)) * 100.0
    );
}

/// Criterion benchmark for lookup performance.
/// All values are inserted into both the trie and they are all looked up
/// again.
/// Tests the performance of map-like use cases.
pub fn bench_lookups<K>(c: &mut Criterion, group_name: &str, keys: &[K])
where
    K: AsRef<[u8]> + Clone + Ord,
{
    let mut trie = TrieMap::new();
    let mut btree = BTreeMap::new();

    for key in keys {
        trie.insert(key, 0usize);
        btree.insert(key.clone(), 0usize);
    }

    let mut group = c.benchmark_group(group_name);
    group.bench_function("TrieMap", |b| {
        b.iter(|| {
            for key in keys {
                black_box(trie.get(key));
            }
        })
    });

    group.bench_function("BTreeMap", |b| {
        b.iter(|| {
            for key in keys {
                black_box(btree.get(key));
            }
        })
    });
    group.finish();
}

/// Criterion benchmark for lookup performance.
/// Tests the performance of insertion operations.
pub fn bench_inserts<K>(c: &mut Criterion, group_name: &str, keys: &[K])
where
    K: AsRef<[u8]> + Clone + Ord,
{
    let mut group = c.benchmark_group(group_name);
    group.bench_function("TrieMap", |b| {
        b.iter(|| {
            let mut trie = TrieMap::new();
            for key in keys {
                trie.insert(key, 0usize);
            }
            black_box(trie);
        })
    });

    group.bench_function("BTreeMap", |b| {
        b.iter(|| {
            let mut btree = BTreeMap::new();
            for key in keys {
                btree.insert(key.clone(), 0usize);
            }
            black_box(btree);
        })
    });
    group.finish();
}

/// Benchmark for remove operations. Adds the given keys and removes them all
pub fn bench_removes<K>(c: &mut Criterion, group_name: &str, keys: &[K])
where
    K: AsRef<[u8]> + Clone + Ord,
{
    let mut group = c.benchmark_group(group_name);
    group.bench_function("TrieMap", |b| {
        b.iter_batched(
            || {
                let mut trie = TrieMap::new();
                for key in keys {
                    trie.insert(key, 0usize);
                }
                trie
            },
            |mut trie| {
                for key in keys {
                    black_box(trie.remove(key));
                }
            },
            BatchSize::LargeInput,
        )
    });

    group.bench_function("BTreeMap", |b| {
        b.iter_batched(
            || {
                let mut btree = BTreeMap::new();
                for key in keys {
                    btree.insert(key.clone(), 0usize);
                }
                btree
            },
            |mut btree| {
                for key in keys {
                    black_box(btree.remove(key));
                }
            },
            BatchSize::LargeInput,
        )
    });
    group.finish();
}

/// Benchmark for prefix removal operations.
pub fn bench_remove_prefix<K>(c: &mut Criterion, group_name: &str, keys: &[K], prefixes: &[K])
where
    K: AsRef<[u8]> + Clone + Ord,
{
    let mut group = c.benchmark_group(group_name);
    group.bench_function("TrieMap", |b| {
        b.iter_batched(
            || {
                let mut trie = TrieMap::new();
                for key in keys {
                    trie.insert(key, 0usize);
                }
                trie
            },
            |mut trie| {
                for prefix in prefixes {
                    black_box(trie.remove_prefix(prefix));
                }
            },
            BatchSize::LargeInput,
        )
    });
    group.finish();
}
