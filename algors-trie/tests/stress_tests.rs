use algors_trie::TrieMap;
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use std::collections::BTreeMap;

#[test]
fn test_basic_ops() {
    let mut trie = TrieMap::new();
    assert!(trie.is_empty());

    trie.insert("apple", 1);
    trie.insert("app", 2);
    trie.insert("banana", 3);

    assert!(!trie.is_empty());
    assert_eq!(trie.get("apple"), Some(&1));
    assert_eq!(trie.get("app"), Some(&2));
    assert_eq!(trie.get("banana"), Some(&3));
    assert_eq!(trie.get("ap"), None);

    assert!(trie.contains("apple"));
    assert!(trie.contains("app"));
    assert!(!trie.contains("ap"));

    *trie.get_mut("apple").unwrap() = 10;
    assert_eq!(trie.get("apple"), Some(&10));

    assert_eq!(trie.remove("apple"), Some(10));
    assert!(!trie.contains("apple"));
    assert!(trie.contains("app"));

    trie.clear();
    assert!(trie.is_empty());
}

#[test]
fn test_prefix_ops() {
    let mut trie = TrieMap::new();
    let keys = ["app", "apple", "apply", "apt", "bat", "bath"];
    for (i, key) in keys.iter().enumerate() {
        trie.insert(key, i);
    }

    // test for_each_prefix
    let mut results = Vec::new();
    trie.for_each_prefix("app", |suffix, val| {
        results.push((suffix.to_vec(), *val));
    });
    results.sort_by_key(|r| r.1);

    assert_eq!(
        results,
        vec![
            (b"".to_vec(), 0),   // "app"
            (b"le".to_vec(), 1), // "apple"
            (b"ly".to_vec(), 2), // "apply"
        ]
    );

    // test empty prefix (should return all)
    let mut all = Vec::new();
    trie.for_each_prefix("", |_, val| all.push(*val));
    assert_eq!(all.len(), keys.len());
}

#[test]
fn test_remove_prefix() {
    let mut trie = TrieMap::new();
    let keys = ["app", "apple", "apply", "apt", "bat", "bath"];
    for (i, key) in keys.iter().enumerate() {
        trie.insert(key, i);
    }

    trie.remove_prefix("app");

    // All "app" prefixed keys should be gone
    assert!(!trie.contains("app"));
    assert!(!trie.contains("apple"));
    assert!(!trie.contains("apply"));

    // "apt" shares "ap" with "app" but does not have prefix "app"
    assert!(trie.contains("apt"));
    assert!(trie.contains("bat"));
    assert!(trie.contains("bath"));

    // Remove empty prefix should clear the trie
    trie.remove_prefix("");
    assert!(trie.is_empty());
}

#[test]
fn test_compression_and_splitting() {
    let mut trie = TrieMap::new();

    // force split
    trie.insert("foobar", 1);
    trie.insert("foobaz", 2);
    // prefix "fooba" should be shared

    assert_eq!(trie.get("foobar"), Some(&1));
    assert_eq!(trie.get("foobaz"), Some(&2));

    // insert in the middle
    trie.insert("foo", 3);
    assert_eq!(trie.get("foo"), Some(&3));

    // remove middle, shouldn't compress because it has 2 children
    trie.remove("foo");
    assert_eq!(trie.get("foobar"), Some(&1));
    assert_eq!(trie.get("foobaz"), Some(&2));

    // remove one child, should compress "fooba" + "r" or "z"
    trie.remove("foobar");
    assert_eq!(trie.get("foobaz"), Some(&2));
}

#[test]
fn test_max_width_branching() {
    let mut trie = TrieMap::new();
    // test that a node can handle all 256 possible byte children
    for i in 0..=255u8 {
        trie.insert(&[i], i as usize);
    }
    for i in 0..=255u8 {
        assert_eq!(trie.get(&[i]), Some(&(i as usize)));
    }
}

#[test]
fn test_random_stress() {
    let mut trie = TrieMap::new();
    let mut reference = BTreeMap::new();
    let mut rng = StdRng::seed_from_u64(42);

    let mut keys = Vec::new();

    // mixed random insert and lookup
    for _ in 0..2000 {
        let op = rng.gen_range(0..3);
        if op == 0 || keys.is_empty() {
            // insert
            let len = rng.gen_range(1..64);
            let mut key = vec![0u8; len];
            rng.fill_bytes(&mut key);
            let val = rng.next_u32();

            trie.insert(&key, val);
            reference.insert(key.clone(), val);
            keys.push(key);
        } else if op == 1 {
            // get
            let idx = rng.gen_range(0..keys.len());
            let key = &keys[idx];
            assert_eq!(trie.get(key), reference.get(key));
        } else {
            // remove
            let idx = rng.gen_range(0..keys.len());
            let key = keys.swap_remove(idx);
            assert_eq!(trie.remove(&key), reference.remove(&key));
        }
    }

    // integrity check against the reference data
    for (key, val) in &reference {
        assert_eq!(trie.get(key), Some(val));
    }
}

#[test]
fn test_deep_tree_compression() {
    let mut trie = TrieMap::new();
    let mut key = Vec::new();

    // make a deep chain of single-child nodes
    for i in 0..100 {
        key.push(b'a');
        trie.insert(&key, i);
    }

    // Remove bottom up, so compression happens
    for i in (0..100usize).rev() {
        assert_eq!(trie.remove(&key), Some(i));
        key.pop();
        // After removing, the remaining tree should still be valid
        if !key.is_empty() {
            assert_eq!(trie.get(&key), Some(&(i.saturating_sub(1))));
        }
    }
    assert!(trie.is_empty());
}
