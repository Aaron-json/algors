/// A Disjoint Set Union / Union-Find implementation.
pub struct UnionFind {
    // Negative: root. abs(value) is the size of the set.
    // Positive: child. value is the index of the parent.
    data: Vec<isize>,
    num_sets: usize,
}

impl UnionFind {
    /// Creates a new Union-Find structure with `size` elements,
    /// each in its own set.
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![-1; size],
            num_sets: size,
        }
    }

    /// Creates a new empty Union-Find structure with space for
    /// `size` elements preallocated.
    pub fn with_capacity(size: usize) -> Self {
        Self {
            data: Vec::with_capacity(size),
            num_sets: 0,
        }
    }

    /// Inserts a new node in its own set and returns the index of the
    /// new added element.
    ///
    /// Might trigger an allocation if capacity is reached.
    pub fn add(&mut self) -> usize {
        let new_i = self.data.len();
        self.data.push(-1);
        self.num_sets += 1;

        new_i
    }

    /// Finds the representative (root) of the set containing `i`.
    ///
    /// Each call flattens the tree.
    pub fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while self.data[root] >= 0 {
            root = self.data[root] as usize;
        }

        let mut cur = i;
        while self.data[cur] >= 0 {
            let next = self.data[cur];
            self.data[cur] = root as isize;
            cur = next as usize;
        }

        root
    }

    /// Merges the sets containing `i` and `j`.
    ///
    /// Returns whether the sets were not already merged.
    pub fn union(&mut self, i: usize, j: usize) -> bool {
        let root_i = self.find(i);
        let root_j = self.find(j);

        if root_j == root_i {
            return false;
        };

        if self.data[root_j] <= self.data[root_i] {
            self.data[root_j] += self.data[root_i];
            self.data[root_i] = root_j as isize;
        } else {
            self.data[root_i] += self.data[root_j];
            self.data[root_j] = root_i as isize;
        }

        self.num_sets -= 1;

        return true;
    }

    /// Returns the number of elements in the set containing `i`.
    pub fn size_of(&mut self, i: usize) -> usize {
        let root = self.find(i);
        self.data[root].unsigned_abs()
    }

    /// Returns the total number of disjoint sets.
    pub fn num_sets(&self) -> usize {
        self.num_sets
    }

    /// Returns the total number of elements.
    pub fn len(&self) -> usize {
        self.data.len()
    }
}
