//! Ring buffer for bounded event storage in live capture mode.

use std::collections::VecDeque;

/// A ring buffer with a fixed capacity that evicts oldest items when full.
///
/// Used in live capture mode to maintain a sliding window of recent events
/// without unbounded memory growth.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    data: VecDeque<T>,
    capacity: usize,
    total_pushed: u64,
    evicted: u64,
}

impl<T> RingBuffer<T> {
    /// Create a new ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
            total_pushed: 0,
            evicted: 0,
        }
    }

    /// Push an item into the buffer, evicting the oldest if at capacity.
    pub fn push(&mut self, item: T) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
            self.evicted += 1;
        }
        self.data.push_back(item);
        self.total_pushed += 1;
    }

    /// Get the current number of items in the buffer.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the total number of items pushed (including evicted).
    pub fn total_pushed(&self) -> u64 {
        self.total_pushed
    }

    /// Get the total number of items evicted.
    pub fn evicted(&self) -> u64 {
        self.evicted
    }

    /// Iterate over items in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    /// Get an item by index.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.data.clear();
        self.total_pushed = 0;
        self.evicted = 0;
    }

    /// Get the maximum capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buf = RingBuffer::new(3);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.capacity(), 3);

        buf.push(1);
        buf.push(2);
        buf.push(3);

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.total_pushed(), 3);
        assert_eq!(buf.evicted(), 0);
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let mut buf = RingBuffer::new(3);

        buf.push(1);
        buf.push(2);
        buf.push(3);
        buf.push(4); // Should evict 1

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.total_pushed(), 4);
        assert_eq!(buf.evicted(), 1);

        let items: Vec<_> = buf.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4]);
    }

    #[test]
    fn test_ring_buffer_get() {
        let mut buf = RingBuffer::new(5);
        buf.push(10);
        buf.push(20);
        buf.push(30);

        assert_eq!(buf.get(0), Some(&10));
        assert_eq!(buf.get(1), Some(&20));
        assert_eq!(buf.get(2), Some(&30));
        assert_eq!(buf.get(3), None);
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut buf = RingBuffer::new(5);
        buf.push(1);
        buf.push(2);
        buf.push(3);

        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.total_pushed(), 0);
        assert_eq!(buf.evicted(), 0);
    }
}
