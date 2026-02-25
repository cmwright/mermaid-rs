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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_list_is_empty() {
        let list: List<i32> = List::new();
        assert!(list.is_empty());
    }

    #[test]
    fn enqueue_dequeue_fifo_order() {
        let mut list = List::new();
        list.enqueue(1);
        list.enqueue(2);
        list.enqueue(3);
        // enqueue pushes to front, dequeue pops from back => FIFO
        assert_eq!(list.dequeue(), Some(1));
        assert_eq!(list.dequeue(), Some(2));
        assert_eq!(list.dequeue(), Some(3));
        assert_eq!(list.dequeue(), None);
    }

    #[test]
    fn dequeue_from_empty_returns_none() {
        let mut list: List<String> = List::new();
        assert_eq!(list.dequeue(), None);
    }

    #[test]
    fn is_empty_after_drain() {
        let mut list = List::new();
        list.enqueue("a");
        assert!(!list.is_empty());
        list.dequeue();
        assert!(list.is_empty());
    }

    #[test]
    fn default_creates_empty_list() {
        let list: List<u8> = List::default();
        assert!(list.is_empty());
    }

    #[test]
    fn interleaved_enqueue_dequeue() {
        let mut list = List::new();
        list.enqueue(10);
        list.enqueue(20);
        assert_eq!(list.dequeue(), Some(10));
        list.enqueue(30);
        assert_eq!(list.dequeue(), Some(20));
        assert_eq!(list.dequeue(), Some(30));
        assert!(list.is_empty());
    }
}
