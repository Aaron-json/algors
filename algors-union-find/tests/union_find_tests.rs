use algors_union_find::UnionFind;

#[test]
fn test_new_initialization() {
    // check initialization state
    let n = 10;
    let mut uf = UnionFind::new(n);
    assert_eq!(uf.len(), n);
    assert_eq!(uf.num_sets(), n);
    for i in 0..n {
        assert_eq!(uf.size_of(i), 1);
    }
}

#[test]
fn test_with_capacity_and_add() {
    // Checks initialization with capacity and element addition
    let mut uf = UnionFind::with_capacity(10);
    assert_eq!(uf.len(), 0);
    assert_eq!(uf.num_sets(), 0);

    let id0 = uf.add();
    let id1 = uf.add();
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(uf.len(), 2);
    assert_eq!(uf.num_sets(), 2);
}

#[test]
fn test_union_basic() {
    // union checks.
    let mut uf = UnionFind::new(10);

    assert!(uf.union(0, 1));
    assert!(uf.union(2, 3));
    assert!(uf.union(4, 5));
    assert_eq!(uf.num_sets(), 7);

    assert_eq!(uf.find(0), uf.find(1));
    assert_eq!(uf.find(2), uf.find(3));
    assert_ne!(uf.find(0), uf.find(2));

    // Redundant union should return false.
    assert!(!uf.union(1, 0));
    assert_eq!(uf.num_sets(), 7);
}

#[test]
fn test_union_by_size_logic() {
    // Ensures the smaller tree is attached to the larger tree.
    let mut uf = UnionFind::new(10);

    // Set of 3
    uf.union(0, 1);
    uf.union(0, 2);

    // Set of 2
    uf.union(3, 4);

    assert_eq!(uf.size_of(0), 3);
    assert_eq!(uf.size_of(3), 2);

    let root_large = uf.find(0);
    uf.union(0, 3);

    assert_eq!(uf.find(3), root_large);
    assert_eq!(uf.size_of(0), 5);
}

#[test]
fn test_path_compression() {
    // Verifies that find flattens the tree.
    let mut uf = UnionFind::new(100);

    for i in 0..99 {
        uf.union(i, i + 1);
    }

    let root = uf.find(0);
    for i in 0..100 {
        assert_eq!(uf.find(i), root);
    }
}

#[test]
fn test_self_union() {
    // Union with self should be a no-op.
    let mut uf = UnionFind::new(5);
    assert!(!uf.union(2, 2));
    assert_eq!(uf.num_sets(), 5);
}
