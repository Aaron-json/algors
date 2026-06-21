use core::cmp;

/// Implements a minimum binary heap.
pub struct Heap<T, F> {
    data: Vec<T>,
    cmp: F,
}

impl<T, F> Heap<T, F>
where
    F: FnMut(&T, &T) -> cmp::Ordering,
{
    pub fn new(cmp: F) -> Self {
        Self {
            data: Vec::new(),
            cmp,
        }
    }

    /// Returns the number of items currently in the heap.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns if the heap has no elements. (ie. length is 0)
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the index of the left child for a given index
    #[inline]
    fn left(i: usize) -> usize {
        (i * 2) + 1
    }

    /// Returns the index of the right child for a given index
    #[inline]
    fn right(i: usize) -> usize {
        (i * 2) + 2
    }

    /// Returns the index of the parent for a given index
    #[inline]
    fn parent(i: usize) -> usize {
        (i - 1) / 2
    }

    pub fn push(&mut self, element: T) {
        self.data.push(element);
        self.sift_up();
    }

    /// Returns the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        // swap the root with the last value
        let last_idx = self.data.len() - 1;
        self.data.swap(0, last_idx);

        let res = self.data.pop();
        self.sift_down();
        res
    }

    pub fn peek(&self) -> Option<&T> {
        self.data.get(0)
    }

    /// Given a valid heap representation except for the last element, this
    /// method places the last element into its correct position
    fn sift_up(&mut self) {
        if self.len() <= 1 {
            return;
        }
        let mut i = self.data.len() - 1;

        while i > 0 {
            let p = Self::parent(i);
            if let cmp::Ordering::Less | cmp::Ordering::Equal =
                (self.cmp)(&self.data[p], &self.data[i])
            {
                break;
            }
            self.data.swap(i, p);
            i = p;
        }
    }

    fn sift_down(&mut self) {
        if self.len() <= 1 {
            return;
        }

        let mut i = 0;

        while i < self.len() {
            let left = Self::left(i);
            if left >= self.len() {
                break;
            }
            let mut child = left;
            let right = Self::right(i);
            // compare with the smaller child
            if right < self.len()
                && let cmp::Ordering::Less = (self.cmp)(&self.data[right], &self.data[child])
            {
                child = right;
            };
            if let cmp::Ordering::Greater = (self.cmp)(&self.data[i], &self.data[child]) {
                self.data.swap(i, child);
            } else {
                break;
            }
            i = child;
        }
    }
}
