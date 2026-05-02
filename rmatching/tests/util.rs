use rmatching::util::arena::Arena;
use rmatching::util::radix_heap::{HasTime, RadixHeapQueue};
use rmatching::util::varying::*;
use std::num::Wrapping;

// ---- Varying tests ----

#[test]
fn varying_growing() {
    let v = VaryingCT::growing_varying_with_zero_distance_at_time(5);
    assert_eq!(v.get_distance_at_time(5), 0);
    assert_eq!(v.get_distance_at_time(10), 5);
    assert!(v.is_growing());
    assert!(!v.is_frozen());
    assert!(!v.is_shrinking());
}

#[test]
fn varying_frozen() {
    let v = VaryingCT::frozen(10);
    assert_eq!(v.get_distance_at_time(0), 10);
    assert_eq!(v.get_distance_at_time(100), 10);
    assert!(v.is_frozen());
    assert!(!v.is_growing());
    assert!(!v.is_shrinking());
}

#[test]
fn varying_shrinking_intercept() {
    // shrinking with y_intercept = 20
    let v = Varying::<i64>((20i64 << 2) | 2);
    assert!(v.is_shrinking());
    assert_eq!(v.y_intercept(), 20);
    assert_eq!(v.time_of_x_intercept(), 20);
    assert_eq!(v.get_distance_at_time(5), 15);
    assert_eq!(v.get_distance_at_time(20), 0);
}

#[test]
fn varying_growing_intercept() {
    // growing with y_intercept = -10 => reaches 0 at time 10
    let v = Varying::<i64>((-10i64 << 2) | 1);
    assert!(v.is_growing());
    assert_eq!(v.time_of_x_intercept(), 10);
}

#[test]
#[should_panic(expected = "frozen varying has no x-intercept")]
fn varying_frozen_intercept_panics() {
    let v = VaryingCT::frozen(10);
    v.time_of_x_intercept();
}

#[test]
fn varying_state_transition() {
    let v = VaryingCT::growing_varying_with_zero_distance_at_time(0);
    assert_eq!(v.get_distance_at_time(5), 5);

    // Freeze at time 5 => stays at distance 5 forever.
    let frozen = v.then_frozen_at_time(5);
    assert!(frozen.is_frozen());
    assert_eq!(frozen.get_distance_at_time(10), 5);
    assert_eq!(frozen.get_distance_at_time(100), 5);

    // Shrink from frozen at time 10 => distance = 5 - (t - 10) = 15 - t
    let shrinking = frozen.then_shrinking_at_time(10);
    assert!(shrinking.is_shrinking());
    assert_eq!(shrinking.get_distance_at_time(10), 5);
    assert_eq!(shrinking.get_distance_at_time(15), 0);
}

#[test]
fn varying_add_sub() {
    let v = VaryingCT::frozen(10);
    let v2 = v + 5i64;
    assert_eq!(v2.y_intercept(), 15);
    assert!(v2.is_frozen());

    let v3 = v2 - 3i64;
    assert_eq!(v3.y_intercept(), 12);
}

#[test]
fn varying_collision_time() {
    // Two regions both growing from time 0, weight 10 between them.
    // collision_time = (weight - y_int_a - y_int_b) / 2 = 10/2 = 5
    let a = VaryingCT::growing_varying_with_zero_distance_at_time(0);
    let b = VaryingCT::growing_varying_with_zero_distance_at_time(0);
    assert_eq!(a.y_intercept(), 0);
    assert_eq!(b.y_intercept(), 0);

    // Using time_of_x_intercept_when_added_to:
    // We want to find when weight_remaining = weight - rad_a - rad_b = 0
    // Represent weight_remaining as a Varying: frozen(10) + growing_a_yint + growing_b_yint
    // But the API works on the sum of two varyings, so let's test directly.
    let sum_intercept = a.time_of_x_intercept_when_added_to(b);
    // Both growing from y_int=0, so neg_sum = 0, collision at 0.
    assert_eq!(sum_intercept, 0);

    // More interesting: growing from time 0 means y_int = 0.
    // If we add weight 10 to one: a2 has y_int = -10 (still growing).
    let a2 = a - 5i64; // y_int = -5
    let b2 = b - 5i64; // y_int = -5
    // neg_sum = -(-5) - (-5) = 10, both growing => 10/2 = 5
    assert_eq!(a2.time_of_x_intercept_when_added_to(b2), 5);
}

#[test]
fn varying_colliding_with() {
    let growing = VaryingCT::growing_varying_with_zero_distance_at_time(0);
    let frozen = VaryingCT::frozen(10);
    let shrinking = Varying::<i64>((10i64 << 2) | 2);

    // growing + frozen => colliding (one growing, combined slope bits = 0b01)
    assert!(growing.colliding_with(frozen));
    assert!(frozen.colliding_with(growing));

    // growing + growing => colliding
    assert!(growing.colliding_with(growing));

    // growing + shrinking => NOT colliding (bits = 0b01 | 0b10 = 0b11 != 0b01)
    assert!(!growing.colliding_with(shrinking));

    // frozen + frozen => NOT colliding
    assert!(!frozen.colliding_with(frozen));
}

#[test]
fn varying_i32() {
    let v = Varying32::frozen(42);
    assert_eq!(v.y_intercept(), 42);
    assert_eq!(v.get_distance_at_time(999), 42);
}

// ---- Arena tests ----

#[test]
fn arena_alloc_free_reuse() {
    let mut arena: Arena<i32> = Arena::new();
    let a = arena.alloc();
    let b = arena.alloc();
    assert_ne!(a, b);

    arena.free(a);
    let c = arena.alloc();
    assert_eq!(c, a); // reused

    // The reused slot should be reset to default.
    assert_eq!(*arena.get(c), 0);
}

#[test]
fn arena_get_set() {
    let mut arena: Arena<String> = Arena::new();
    let idx = arena.alloc();
    *arena.get_mut(idx) = "hello".to_string();
    assert_eq!(arena.get(idx), "hello");
    assert_eq!(&arena[idx], "hello");
}

#[test]
fn arena_clear() {
    let mut arena: Arena<u64> = Arena::new();
    arena.alloc();
    arena.alloc();
    assert_eq!(arena.len(), 2);
    arena.clear();
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
}

// ---- RadixHeapQueue tests ----

/// Minimal event type for testing.
#[derive(Debug, Clone)]
struct TestEvent {
    time: Wrapping<u32>,
    payload: u32,
}

impl HasTime for TestEvent {
    fn time(&self) -> Wrapping<u32> {
        self.time
    }
    fn no_event() -> Self {
        TestEvent {
            time: Wrapping(u32::MAX),
            payload: u32::MAX,
        }
    }
    fn is_no_event(&self) -> bool {
        self.payload == u32::MAX
    }
}

#[test]
fn radix_heap_empty() {
    let mut q: RadixHeapQueue<TestEvent> = RadixHeapQueue::new();
    assert!(q.is_empty());
    let e = q.dequeue();
    assert!(e.is_no_event());
}

#[test]
fn radix_heap_single() {
    let mut q: RadixHeapQueue<TestEvent> = RadixHeapQueue::new();
    q.enqueue(TestEvent {
        time: Wrapping(5),
        payload: 42,
    });
    assert_eq!(q.len(), 1);

    let e = q.dequeue();
    assert_eq!(e.payload, 42);
    assert_eq!(e.time, Wrapping(5));
    assert!(q.is_empty());
}

#[test]
fn radix_heap_ordering() {
    let mut q: RadixHeapQueue<TestEvent> = RadixHeapQueue::new();
    // Insert out of order.
    for &(t, p) in &[(10u32, 1u32), (3, 2), (7, 3), (1, 4), (20, 5)] {
        q.enqueue(TestEvent {
            time: Wrapping(t),
            payload: p,
        });
    }
    assert_eq!(q.len(), 5);

    // Should come out in time order.
    let mut prev_time = 0u32;
    for _ in 0..5 {
        let e = q.dequeue();
        assert!(!e.is_no_event());
        assert!(e.time.0 >= prev_time);
        prev_time = e.time.0;
    }
    assert!(q.is_empty());
}

#[test]
fn radix_heap_same_time() {
    let mut q: RadixHeapQueue<TestEvent> = RadixHeapQueue::new();
    for i in 0..5 {
        q.enqueue(TestEvent {
            time: Wrapping(10),
            payload: i,
        });
    }
    let mut payloads = Vec::new();
    while !q.is_empty() {
        payloads.push(q.dequeue().payload);
    }
    assert_eq!(payloads.len(), 5);
    payloads.sort();
    assert_eq!(payloads, vec![0, 1, 2, 3, 4]);
}

#[test]
fn radix_heap_reset() {
    let mut q: RadixHeapQueue<TestEvent> = RadixHeapQueue::new();
    q.enqueue(TestEvent {
        time: Wrapping(5),
        payload: 1,
    });
    q.reset();
    assert!(q.is_empty());
    assert_eq!(q.cur_time, 0);
}

// ---------------------------------------------------------------------------
// Coverage: Arena Default trait (lines 63-64)
// ---------------------------------------------------------------------------

#[test]
fn arena_default_trait() {
    let arena: Arena<i32> = Arena::default();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

// ---------------------------------------------------------------------------
// Coverage: RadixHeapQueue Default trait (lines 117-119)
// ---------------------------------------------------------------------------

#[test]
fn radix_heap_default_trait() {
    let q: RadixHeapQueue<TestEvent> = RadixHeapQueue::default();
    assert!(q.is_empty());
    assert_eq!(q.cur_time, 0);
}

// ---------------------------------------------------------------------------
// Coverage: RadixHeapQueue clear (line 70 - dequeue when non-empty
// but bucket 0 empty and all other buckets empty too)
// ---------------------------------------------------------------------------

#[test]
fn radix_heap_clear_then_dequeue() {
    let mut q: RadixHeapQueue<TestEvent> = RadixHeapQueue::new();
    q.enqueue(TestEvent {
        time: Wrapping(5),
        payload: 1,
    });
    q.clear();
    // After clear, dequeue should return no_event
    let e = q.dequeue();
    assert!(e.is_no_event());
}

// ---------------------------------------------------------------------------
// Coverage: Varying<i32> VaryingInt trait methods (lines 37-38)
// ---------------------------------------------------------------------------

#[test]
fn varying_i32_two_and_three() {
    // Test Varying<i32> shrinking (uses two()) and growing+shrinking combo
    let shrinking = Varying::<i32>((15i32 << 2) | 2);
    assert!(shrinking.is_shrinking());
    assert_eq!(shrinking.y_intercept(), 15);
    assert_eq!(shrinking.get_distance_at_time(5), 10);
    assert_eq!(shrinking.time_of_x_intercept(), 15);

    // Test is_frozen which checks (data & three()) == zero()
    let frozen = Varying32::frozen(7);
    assert!(frozen.is_frozen());
    assert_eq!(frozen.y_intercept(), 7);
}

// ---------------------------------------------------------------------------
// Coverage: Varying time_of_x_intercept_when_added_to with mixed slopes (line 129)
// ---------------------------------------------------------------------------

#[test]
fn varying_x_intercept_mixed_slopes() {
    // One growing, one frozen: combined slope = 1, not 2
    let growing = VaryingCT::growing_varying_with_zero_distance_at_time(0); // y_int = 0
    let frozen = VaryingCT::frozen(0); // y_int = 0

    // neg_sum = 0 - 0 - 0 = 0; not both growing => result = 0
    let t = growing.time_of_x_intercept_when_added_to(frozen);
    assert_eq!(t, 0);

    // growing from time 0 with offset: y_int = -5
    let g = growing - 5i64; // y_int = -5, growing
    let f = VaryingCT::frozen(0); // y_int = 0

    // neg_sum = -(-5) - 0 = 5; not both growing => result = 5
    let t = g.time_of_x_intercept_when_added_to(f);
    assert_eq!(t, 5);
}

// ---------------------------------------------------------------------------
// Coverage: Varying state transitions for i32
// ---------------------------------------------------------------------------

#[test]
fn varying_i32_state_transitions() {
    let v = Varying32::growing_varying_with_zero_distance_at_time(0);
    assert!(v.is_growing());
    assert_eq!(v.get_distance_at_time(10), 10);

    let frozen = v.then_frozen_at_time(10);
    assert!(frozen.is_frozen());
    assert_eq!(frozen.get_distance_at_time(100), 10);

    let shrinking = frozen.then_shrinking_at_time(10);
    assert!(shrinking.is_shrinking());
    assert_eq!(shrinking.get_distance_at_time(15), 5);

    let growing_again = shrinking.then_growing_at_time(15);
    assert!(growing_again.is_growing());
    assert_eq!(growing_again.get_distance_at_time(15), 5);
    assert_eq!(growing_again.get_distance_at_time(20), 10);
}
