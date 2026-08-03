pub(crate) struct PqEntry {
    /// Index into the solver's link arrays (integer arena), not a reference,
    /// so priority updates and comparisons avoid string/pointer chasing.
    pub(crate) link: usize,
    /// u_j + c_a (used for prioritization)
    pub(crate) priority: f64,
    /// The index of the item in the heap, -1 when popped
    pub(crate) index: i64,
}

/// Binary min-heap over links keyed by u_j + c_a.
/// Entries live in an arena and are referred to by their arena id.
pub(crate) struct PriorityQueue {
    /// Arena of all entries; an entry id is its position in this vector
    entries: Vec<PqEntry>,
    /// Heap of entry ids ordered by priority
    heap: Vec<usize>,
}

impl PriorityQueue {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        PriorityQueue {
            entries: Vec::with_capacity(capacity),
            heap: Vec::with_capacity(capacity),
        }
    }

    /// Appends a new entry without restoring the heap property.
    /// Index fields are assigned later by `init`.
    pub(crate) fn push(&mut self, link: usize, priority: f64) -> usize {
        let id = self.entries.len();
        self.entries.push(PqEntry {
            link,
            priority,
            index: 0,
        });
        self.heap.push(id);
        id
    }

    pub(crate) fn len(&self) -> usize {
        self.heap.len()
    }

    /// Empties the queue while keeping the backing capacity, so a Workspace
    /// can reuse it across destinations without reallocating.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.heap.clear();
    }

    pub(crate) fn link(&self, id: usize) -> usize {
        self.entries[id].link
    }

    pub(crate) fn priority(&self, id: usize) -> f64 {
        self.entries[id].priority
    }

    fn less(&self, i: usize, j: usize) -> bool {
        self.entries[self.heap[i]].priority <= self.entries[self.heap[j]].priority
    }

    fn swap(&mut self, i: usize, j: usize) {
        self.heap.swap(i, j);
        self.entries[self.heap[i]].index = i as i64;
        self.entries[self.heap[j]].index = j as i64;
    }

    /// Pops the minimum entry and returns its id
    pub(crate) fn pop(&mut self) -> Option<usize> {
        let n = self.heap.len();
        if n == 0 {
            return None;
        }
        let first = self.heap[0];
        let last = self.heap[n - 1];
        self.heap[0] = last;
        self.entries[last].index = 0;
        self.heap.truncate(n - 1);
        self.sift_down(0);
        self.entries[first].index = -1;
        Some(first)
    }

    /// Establishes the heap property over all pushed entries
    pub(crate) fn init(&mut self) {
        for i in 0..self.heap.len() {
            self.entries[self.heap[i]].index = i as i64;
        }
        let mut i = self.heap.len() as i64 / 2 - 1;
        while i >= 0 {
            self.sift_down(i as usize);
            i -= 1;
        }
    }

    /// update modifies the priority of an entry in the queue
    pub(crate) fn update(&mut self, id: usize, priority: f64) {
        if self.entries[id].index < 0 {
            return;
        }
        let old_priority = self.entries[id].priority;
        self.entries[id].priority = priority;
        let index = self.entries[id].index;
        if priority <= old_priority {
            self.sift_up(index);
        } else {
            self.sift_down(index as usize);
        }
    }

    /// sift_up moves an element up the heap until heap property is satisfied
    fn sift_up(&mut self, mut i: i64) {
        while i > 0 {
            let parent = (i - 1) / 2;
            if self.less(i as usize, parent as usize) {
                self.swap(i as usize, parent as usize);
                i = parent;
            } else {
                break;
            }
        }
    }

    /// sift_down moves an element down the heap until heap property is satisfied
    fn sift_down(&mut self, i: usize) {
        let n = self.heap.len();
        let mut i = i;
        loop {
            let mut smallest = i;
            let left = 2 * i + 1;
            let right = 2 * i + 2;

            if left < n && self.less(left, smallest) {
                smallest = left;
            }
            if right < n && self.less(right, smallest) {
                smallest = right;
            }

            if smallest != i {
                self.swap(i, smallest);
                i = smallest;
            } else {
                break;
            }
        }
    }

    pub(crate) fn print(&self) {
        if self.len() == 0 {
            println!("Priority Queue: <empty>");
            return;
        }
        let arr: Vec<String> = self
            .heap
            .iter()
            .map(|&id| {
                let entry = &self.entries[id];
                format!("link#{} == {:.2}", entry.link, entry.priority)
            })
            .collect();
        println!("Priority Queue: [{}]\\\\ ", arr.join(", "));
    }
}
