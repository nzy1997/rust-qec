use std::num::Wrapping;

const CYCLIC_HALF_RANGE: u32 = (u32::MAX >> 1) + 1;

#[inline]
pub(crate) fn cyclic_lt(a: Wrapping<u32>, b: Wrapping<u32>) -> bool {
    b.0.wrapping_sub(a.0).wrapping_sub(1) < CYCLIC_HALF_RANGE - 1
}

#[inline]
pub(crate) fn cyclic_gt(a: Wrapping<u32>, b: Wrapping<u32>) -> bool {
    a.0.wrapping_sub(b.0).wrapping_sub(1) < CYCLIC_HALF_RANGE - 1
}

#[inline]
pub(crate) fn widen_from_nearby_reference(time: Wrapping<u32>, reference: i64) -> i64 {
    let cyclic_reference = Wrapping(reference as u32);
    let mut result = reference + i64::from(time.0.wrapping_sub(cyclic_reference.0));
    if cyclic_gt(cyclic_reference, time) {
        result -= i64::from(u32::MAX) + 1;
    }
    result
}

/// Trait for event types stored in the radix heap.
///
/// The queue is monotonic: events are dequeued in non-decreasing time order.
/// `time()` returns the cyclic timestamp used for bucket placement.
pub trait HasTime {
    fn time(&self) -> Wrapping<u32>;
    /// Sentinel value representing "no event".
    fn no_event() -> Self;
    fn is_no_event(&self) -> bool;
}

/// 33-bucket monotonic radix-heap priority queue.
///
/// Bucket index for a time `t` is `32 - (t ^ cur_time).leading_zeros()`,
/// so bucket 0 holds events whose time equals `cur_time`, and bucket 32
/// holds the most distant events.
///
/// Invariant: `cur_time` only moves forward (monotonically).
pub struct RadixHeapQueue<E: HasTime> {
    buckets: [Vec<E>; 33],
    pub cur_time: i64,
    num_enqueued: usize,
}

impl<E: HasTime> RadixHeapQueue<E> {
    pub fn new() -> Self {
        RadixHeapQueue {
            buckets: std::array::from_fn(|_| Vec::new()),
            cur_time: 0,
            num_enqueued: 0,
        }
    }

    #[inline]
    fn bucket_for(&self, time: Wrapping<u32>) -> usize {
        let diff = time.0 ^ (self.cur_time as u32);
        if diff == 0 {
            0
        } else {
            (32 - diff.leading_zeros()) as usize
        }
    }

    /// Enqueue an event. Its time must be >= cur_time (monotonic invariant).
    pub fn enqueue(&mut self, event: E) {
        debug_assert!(
            !cyclic_lt(event.time(), Wrapping(self.cur_time as u32)),
            "attempted to enqueue event in the cyclic past: cur_time={} event_time={}",
            self.cur_time,
            event.time().0,
        );
        let bucket = self.bucket_for(event.time());
        self.buckets[bucket].push(event);
        self.num_enqueued += 1;
    }

    /// Dequeue the event with the smallest time.
    ///
    /// Returns `E::no_event()` if the queue is empty.
    pub fn dequeue(&mut self) -> E {
        if self.num_enqueued == 0 {
            return E::no_event();
        }

        // Fast path: bucket 0 has events at exactly cur_time.
        if let Some(event) = self.buckets[0].pop() {
            self.num_enqueued -= 1;
            return event;
        }

        // Find the first non-empty bucket.
        let bi = match self.buckets[1..].iter().position(|b| !b.is_empty()) {
            Some(i) => i + 1,
            None => return E::no_event(),
        };

        if bi == 1 {
            self.buckets.swap(0, 1);
            self.cur_time += 1;
        } else {
            // Find the minimum cyclic time in that bucket and widen it near cur_time.
            let min_time = self.buckets[bi]
                .iter()
                .map(|e| e.time())
                .min_by_key(|t| t.0)
                .unwrap();
            self.cur_time = widen_from_nearby_reference(min_time, self.cur_time);

            // Redistribute all events from this bucket into lower buckets.
            let mut events = Vec::new();
            std::mem::swap(&mut events, &mut self.buckets[bi]);
            for event in events.drain(..) {
                let new_bucket = self.bucket_for(event.time());
                debug_assert!(new_bucket < bi);
                self.buckets[new_bucket].push(event);
            }
            self.buckets[bi] = events;
        }

        // Now bucket 0 must have at least one event.
        self.num_enqueued -= 1;
        self.buckets[0].pop().unwrap()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num_enqueued == 0
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.num_enqueued
    }

    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.num_enqueued = 0;
    }

    pub fn reset(&mut self) {
        self.clear();
        self.cur_time = 0;
    }
}

impl<E: HasTime> Default for RadixHeapQueue<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_alloc::{allocation_count, reset_allocation_count};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TestEvent {
        NoEvent,
        At(Wrapping<u32>),
    }

    impl HasTime for TestEvent {
        fn time(&self) -> Wrapping<u32> {
            match self {
                TestEvent::NoEvent => Wrapping(0),
                TestEvent::At(time) => *time,
            }
        }

        fn no_event() -> Self {
            TestEvent::NoEvent
        }

        fn is_no_event(&self) -> bool {
            matches!(self, TestEvent::NoEvent)
        }
    }

    #[test]
    fn dequeue_preserves_monotonic_time() {
        let mut q = RadixHeapQueue::<TestEvent>::new();
        for t in [9u32, 3, 17, 18, 19, 24, 31] {
            q.enqueue(TestEvent::At(Wrapping(t)));
        }

        let mut widened = Vec::new();
        while !q.is_empty() {
            let event = q.dequeue();
            widened.push(widen_from_nearby_reference(event.time(), q.cur_time));
        }

        assert_eq!(widened, vec![3, 9, 17, 18, 19, 24, 31]);
        assert_eq!(q.cur_time, 31);
    }

    #[test]
    fn cyclic_widen_stays_near_reference() {
        let reference = 4_300_000_000i64;
        let widened =
            widen_from_nearby_reference(Wrapping((reference as u32).wrapping_add(1_000)), reference);
        assert!(widened > reference);
        assert_eq!(widened - reference, 1_000);
    }

    #[test]
    fn dequeue_redistribute_reuses_bucket_storage() {
        let mut q = RadixHeapQueue::<TestEvent>::new();

        for t in [9u32, 3, 17] {
            q.enqueue(TestEvent::At(Wrapping(t)));
        }
        while !q.is_empty() {
            let _ = q.dequeue();
        }

        q.reset();
        for t in [9u32, 3, 17] {
            q.enqueue(TestEvent::At(Wrapping(t)));
        }

        reset_allocation_count();
        let _ = q.dequeue();

        assert_eq!(allocation_count(), 0);
    }
}
