//! Single-threaded delivery with explicit lossless/coalesced policy.
//! No application, Iced, callback, or global-state dependencies.

use std::collections::{HashSet, VecDeque};
use std::hash::Hash;

pub(super) trait BusEvent {
    type Key: Eq + Hash;
    /// None preserves every occurrence. Some coalesces only while queued.
    fn coalescing_key(&self) -> Option<Self::Key>;
}

#[derive(Debug)]
pub(super) struct MessageBus<E: BusEvent> {
    queue: VecDeque<E>,
    queued: HashSet<E::Key>,
}

impl<E: BusEvent> Default for MessageBus<E> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            queued: HashSet::new(),
        }
    }
}

impl<E: BusEvent> MessageBus<E> {
    pub(super) fn publish(&mut self, event: E) {
        if let Some(key) = event.coalescing_key() {
            if !self.queued.insert(key) {
                return;
            }
        }
        self.queue.push_back(event);
    }

    pub(super) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub(super) fn pop(&mut self) -> Option<E> {
        let event = self.queue.pop_front()?;
        if let Some(key) = event.coalescing_key() {
            self.queued.remove(&key);
        }
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, PartialEq)]
    enum TestEvent {
        Refresh(u8),
        Closed(u8),
    }
    impl BusEvent for TestEvent {
        type Key = u8;
        fn coalescing_key(&self) -> Option<u8> {
            match self {
                Self::Refresh(id) => Some(*id),
                Self::Closed(_) => None,
            }
        }
    }
    #[test]
    fn fifo_delivery_coalesces_only_pending_refreshes() {
        let mut bus = MessageBus::default();
        bus.publish(TestEvent::Refresh(1));
        bus.publish(TestEvent::Closed(1));
        bus.publish(TestEvent::Refresh(1));
        bus.publish(TestEvent::Closed(1));
        assert_eq!(bus.pop(), Some(TestEvent::Refresh(1)));
        // A subscriber may enqueue fresh work, delivered after already queued events.
        bus.publish(TestEvent::Refresh(1));
        assert_eq!(bus.pop(), Some(TestEvent::Closed(1)));
        assert_eq!(bus.pop(), Some(TestEvent::Closed(1)));
        assert_eq!(bus.pop(), Some(TestEvent::Refresh(1)));
        assert_eq!(bus.pop(), None);
    }

    #[test]
    fn repeated_delivery_reuses_storage() {
        let mut bus = MessageBus::default();
        bus.publish(TestEvent::Refresh(1));
        bus.pop();
        let capacities = (bus.queue.capacity(), bus.queued.capacity());
        for _ in 0..10_000 {
            bus.publish(TestEvent::Refresh(1));
            bus.publish(TestEvent::Refresh(1));
            assert_eq!(bus.pop(), Some(TestEvent::Refresh(1)));
            assert!(bus.is_empty());
        }
        assert_eq!((bus.queue.capacity(), bus.queued.capacity()), capacities);
    }

    #[test]
    #[ignore = "manual transport microbenchmark; no hardware-dependent pass threshold"]
    fn benchmark_transport() {
        use std::{hint::black_box, time::Instant};
        let mut bus = MessageBus::default();
        let iterations = 1_000_000;
        let start = Instant::now();
        for _ in 0..iterations {
            bus.publish(black_box(TestEvent::Refresh(1)));
            bus.publish(black_box(TestEvent::Refresh(1)));
            bus.publish(black_box(TestEvent::Closed(2)));
            black_box(bus.pop());
            black_box(bus.pop());
        }
        let elapsed = start.elapsed();
        assert!(bus.is_empty());
        eprintln!(
            "{iterations} batches (3 publishes, 2 deliveries, 1 coalesced): {elapsed:?}, {:.1} ns/batch",
            elapsed.as_nanos() as f64 / iterations as f64
        );
    }
}
