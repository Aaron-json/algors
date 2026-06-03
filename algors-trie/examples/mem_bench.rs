use algors_trie::TrieMap;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: TrackingAllocator = TrackingAllocator;

fn load_dictionary() -> Vec<String> {
    let file = File::open("/usr/share/dict/words").expect("Could not open dictionary");
    let reader = BufReader::new(file);
    reader.lines().map_while(Result::ok).take(100_000).collect()
}

fn main() {
    let keys = load_dictionary();
    let num_keys = keys.len();

    // Benchmark BTreeMap
    let base_mem = ALLOCATED.load(Ordering::SeqCst);
    {
        let mut map = BTreeMap::new();
        for key in &keys {
            map.insert(key.clone(), 0u64);
        }
        let used = ALLOCATED.load(Ordering::SeqCst) - base_mem;
        println!(
            "BTreeMap ({} dictionary words): {} bytes ({:.2} MB)",
            num_keys,
            used,
            used as f64 / 1_000_000.0
        );
        println!("  Avg per key: {} bytes", used / num_keys);
    }

    // Benchmark TrieMap
    let base_mem = ALLOCATED.load(Ordering::SeqCst);
    {
        let mut trie = TrieMap::new();
        for key in &keys {
            trie.insert(key, 0u64);
        }
        let used = ALLOCATED.load(Ordering::SeqCst) - base_mem;
        println!(
            "TrieMap ({} dictionary words): {} bytes ({:.2} MB)",
            num_keys,
            used,
            used as f64 / 1_000_000.0
        );
        println!("  Avg per key: {} bytes", used / num_keys);
    }
}
