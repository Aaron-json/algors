use algors_trie::TrieMap;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

#[test]
fn test_basic_ops() {
    let mut trie = TrieMap::new();
    trie.insert("foo", 1);
    trie.insert("fo", 2);
    assert_eq!(trie.get("foo"), Some(&1));
    assert_eq!(trie.get("fo"), Some(&2));
}

#[test]
fn test_prefix_splits() {
    let mut trie = TrieMap::new();
    trie.insert("apple", 1);
    trie.insert("apply", 2);
    trie.insert("app", 3);

    assert_eq!(trie.get("apple"), Some(&1));
    assert_eq!(trie.get("apply"), Some(&2));
    assert_eq!(trie.get("app"), Some(&3));

    assert_eq!(trie.remove("apple"), Some(1));
    assert_eq!(trie.get("apple"), None);
    assert_eq!(trie.get("apply"), Some(&2));
    assert_eq!(trie.get("app"), Some(&3));
}

#[test]
fn test_long_prefixes() {
    let mut trie = TrieMap::new();
    let long_key1 = "a".repeat(100);
    let long_key2 = "a".repeat(100) + "b";

    trie.insert(&long_key1, 1);
    trie.insert(&long_key2, 2);

    assert_eq!(trie.get(&long_key1), Some(&1));
    assert_eq!(trie.get(&long_key2), Some(&2));

    assert_eq!(trie.remove(&long_key1), Some(1));
    assert_eq!(trie.get(&long_key1), None);
    assert_eq!(trie.get(&long_key2), Some(&2));
}

#[test]
fn test_random_stress() {
    let mut trie = TrieMap::new();
    let mut rng = StdRng::seed_from_u64(42);
    let mut keys = Vec::new();

    for _ in 0..1000 {
        let mut key = vec![0u8; (rng.next_u32() % 64) as usize + 1];
        rng.fill_bytes(&mut key);
        trie.insert(&key, key.clone());
        keys.push(key);
    }

    for key in &keys {
        assert_eq!(trie.get(key), Some(key));
    }

    for key in keys.drain(0..500) {
        assert_eq!(trie.remove(&key), Some(key));
    }

    for key in &keys {
        assert_eq!(trie.get(key), Some(key));
    }
}

#[test]
fn test_miri_unsafe_integrity() {
    // This test is designed to be run under Miri.
    // It exercises the backtracking and compression logic in remove.
    let mut trie = TrieMap::new();

    // Create a chain of nodes
    trie.insert("a", 1);
    trie.insert("ab", 2);
    trie.insert("abc", 3);
    trie.insert("abcd", 4);

    // Remove from the end to trigger compression upwards
    assert_eq!(trie.remove("abcd"), Some(4));
    assert_eq!(trie.remove("abc"), Some(3));
    assert_eq!(trie.remove("ab"), Some(2));
    assert_eq!(trie.remove("a"), Some(1));

    assert!(trie.get("a").is_none());
}

#[test]
fn test_radix_set_logic() {
    let mut set = TrieMap::<()>::new();
    let keys = vec!["apple", "app", "application", "aptitude", "bat", "bath"];
    
    for key in &keys {
        set.insert(key, ());
    }

    for key in &keys {
        assert!(set.contains(key));
    }

    assert!(!set.contains("apples"));
    assert!(!set.contains("ap"));

    // Test removals
    assert_eq!(set.remove("app"), Some(()));
    assert!(!set.contains("app"));
    assert!(set.contains("apple")); // Sister node remains
    
    assert_eq!(set.remove("bat"), Some(()));
    assert!(!set.contains("bat"));
    assert!(set.contains("bath")); // Internal node should have compressed
}

#[test]
fn test_max_children() {
    let mut trie = TrieMap::new();
    for i in 0..=255u8 {
        let key = [i];
        trie.insert(&key, i);
    }

    for i in 0..=255u8 {
        let key = [i];
        assert_eq!(trie.get(&key), Some(&i));
    }
}
