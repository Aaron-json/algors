use algors_heap::heap::Heap;
use proptest::prelude::*;

#[test]
fn test_basic_min_heap_operations() {
    let mut heap = Heap::new(|a: &i32, b: &i32| a.cmp(b));
    assert!(heap.is_empty());
    assert_eq!(heap.len(), 0);
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);

    // Push elements
    heap.push(10);
    heap.push(5);
    heap.push(15);
    heap.push(2);
    heap.push(8);

    assert!(!heap.is_empty());
    assert_eq!(heap.len(), 5);
    assert_eq!(heap.peek(), Some(&2));

    // verify ascending order
    assert_eq!(heap.pop(), Some(2));
    assert_eq!(heap.peek(), Some(&5));
    assert_eq!(heap.len(), 4);

    assert_eq!(heap.pop(), Some(5));
    assert_eq!(heap.pop(), Some(8));
    assert_eq!(heap.pop(), Some(10));
    assert_eq!(heap.pop(), Some(15));

    // should be empty
    assert!(heap.is_empty());
    assert_eq!(heap.len(), 0);
    assert_eq!(heap.peek(), None);
    assert_eq!(heap.pop(), None);
}

#[test]
fn test_basic_max_heap_operations() {
    let mut heap = Heap::new(|a: &i32, b: &i32| b.cmp(a));

    heap.push(10);
    heap.push(5);
    heap.push(15);
    heap.push(2);
    heap.push(8);

    assert_eq!(heap.peek(), Some(&15));

    // verify descending order
    assert_eq!(heap.pop(), Some(15));
    assert_eq!(heap.pop(), Some(10));
    assert_eq!(heap.pop(), Some(8));
    assert_eq!(heap.pop(), Some(5));
    assert_eq!(heap.pop(), Some(2));
    assert_eq!(heap.pop(), None);
}

#[test]
fn test_single_element() {
    let mut heap = Heap::new(|a: &i32, b: &i32| a.cmp(b));
    heap.push(42);
    assert_eq!(heap.len(), 1);
    assert_eq!(heap.peek(), Some(&42));
    assert_eq!(heap.pop(), Some(42));
    assert_eq!(heap.pop(), None);
    assert!(heap.is_empty());
}

#[test]
fn test_duplicate_elements() {
    let mut heap = Heap::new(|a: &i32, b: &i32| a.cmp(b));
    let elements = vec![5, 2, 5, 8, 2, 8, 5];
    for &val in &elements {
        heap.push(val);
    }
    assert_eq!(heap.len(), elements.len());

    let mut popped = Vec::new();
    while let Some(val) = heap.pop() {
        popped.push(val);
    }

    let mut expected = elements;
    expected.sort();
    assert_eq!(popped, expected);
}

proptest! {
    /// Tests that the heap sorts correctly compared to a sorted vector.
    #[test]
    fn prop_heap_sorts_correctly(ref vec in prop::collection::vec(any::<i32>(), 0..200)) {
        let mut heap = Heap::new(|a: &i32, b: &i32| a.cmp(b));
        for &val in vec {
            heap.push(val);
        }

        let mut popped = Vec::with_capacity(vec.len());
        while let Some(val) = heap.pop() {
            popped.push(val);
        }

        let mut sorted = vec.clone();
        sorted.sort();
        prop_assert_eq!(popped, sorted);
    }

}
