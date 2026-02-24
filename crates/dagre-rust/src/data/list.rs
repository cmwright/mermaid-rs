//! Simple doubly linked list implementation (deque).
//! Mirrors dagre's `data/list.js`.
//!
//! In JS, list entries are JS objects with `_next`/`_prev` fields.
//! In Rust, we use a VecDeque for simplicity — the JS List is just used as a
//! FIFO queue with the ability to re-enqueue items. VecDeque handles this fine.

use std::collections::VecDeque;

/// A simple queue that mirrors the JS List's enqueue/dequeue semantics.
/// JS List: enqueue pushes to front, dequeue pops from back (FIFO).
#[derive(Debug, Clone)]
pub struct List<T> {
    items: VecDeque<T>,
}

impl<T> List<T> {
    pub fn new() -> Self {
        List {
            items: VecDeque::new(),
        }
    }

    /// Enqueue at front (mirrors JS: entry inserted after sentinel, i.e. at head).
    pub fn enqueue(&mut self, entry: T) {
        self.items.push_front(entry);
    }

    /// Dequeue from back (mirrors JS: removes sentinel._prev, i.e. tail).
    pub fn dequeue(&mut self) -> Option<T> {
        self.items.pop_back()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self::new()
    }
}
