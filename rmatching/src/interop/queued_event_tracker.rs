use std::num::Wrapping;
use crate::util::radix_heap::{cyclic_gt, HasTime, RadixHeapQueue};

#[derive(Debug, Clone)]
pub struct QueuedEventTracker {
    pub desired_time: Wrapping<u32>,
    pub queued_time: Wrapping<u32>,
    pub has_desired_time: bool,
    pub has_queued_time: bool,
}

impl Default for QueuedEventTracker {
    fn default() -> Self {
        QueuedEventTracker {
            desired_time: Wrapping(0),
            queued_time: Wrapping(0),
            has_desired_time: false,
            has_queued_time: false,
        }
    }
}

impl QueuedEventTracker {
    pub fn clear(&mut self) {
        self.has_desired_time = false;
        self.has_queued_time = false;
    }

    /// Schedule a desired event. Only enqueues if no event
    /// is queued or the new event is earlier.
    pub fn set_desired_event<E: HasTime>(
        &mut self,
        event: E,
        queue: &mut RadixHeapQueue<E>,
    ) {
        self.has_desired_time = true;
        self.desired_time = event.time();
        if !self.has_queued_time || cyclic_gt(self.queued_time, event.time()) {
            self.queued_time = event.time();
            self.has_queued_time = true;
            queue.enqueue(event);
        }
    }

    pub fn set_no_desired_event(&mut self) {
        self.has_desired_time = false;
    }

    /// Called when an event is dequeued. Returns true if this event should be processed.
    pub fn dequeue_decision<E: HasTime>(
        &mut self,
        event: &E,
        queue: &mut RadixHeapQueue<E>,
        make_event: impl FnOnce(Wrapping<u32>) -> E,
    ) -> bool {
        // Check if this is the most recent queued event
        if !self.has_queued_time || self.queued_time != event.time() {
            return false; // stale event
        }
        self.has_queued_time = false;

        if !self.has_desired_time {
            return false; // no longer desired
        }

        if self.desired_time != event.time() {
            // Requeue with updated time
            let new_event = make_event(self.desired_time);
            self.queued_time = self.desired_time;
            self.has_queued_time = true;
            queue.enqueue(new_event);
            return false;
        }

        self.has_desired_time = false;
        true
    }
}
