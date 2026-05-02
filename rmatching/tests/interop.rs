use std::num::Wrapping;
use rmatching::types::*;
use rmatching::interop::*;
use rmatching::util::radix_heap::{HasTime, RadixHeapQueue};

#[test]
fn compressed_edge_reversed() {
    let e = CompressedEdge {
        loc_from: Some(NodeIdx(0)),
        loc_to: Some(NodeIdx(1)),
        obs_mask: 0b101,
    };
    let r = e.reversed();
    assert_eq!(r.loc_from, Some(NodeIdx(1)));
    assert_eq!(r.loc_to, Some(NodeIdx(0)));
    assert_eq!(r.obs_mask, 0b101);
}

#[test]
fn compressed_edge_merged() {
    let a = CompressedEdge {
        loc_from: Some(NodeIdx(0)),
        loc_to: Some(NodeIdx(1)),
        obs_mask: 0b101,
    };
    let b = CompressedEdge {
        loc_from: Some(NodeIdx(1)),
        loc_to: Some(NodeIdx(2)),
        obs_mask: 0b110,
    };
    let m = a.merged_with(&b);
    assert_eq!(m.loc_from, Some(NodeIdx(0)));
    assert_eq!(m.loc_to, Some(NodeIdx(2)));
    assert_eq!(m.obs_mask, 0b011); // XOR
}

#[test]
fn compressed_edge_empty() {
    let e = CompressedEdge::empty();
    assert_eq!(e.loc_from, None);
    assert_eq!(e.loc_to, None);
    assert_eq!(e.obs_mask, 0);
}

#[test]
fn mwpm_event_variants() {
    let e = MwpmEvent::NoEvent;
    assert!(e.is_no_event());

    let e2 = MwpmEvent::RegionHitBoundary {
        region: RegionIdx(0),
        edge: CompressedEdge::empty(),
    };
    assert!(!e2.is_no_event());

    let e3 = MwpmEvent::RegionHitRegion {
        region1: RegionIdx(0),
        region2: RegionIdx(1),
        edge: CompressedEdge::empty(),
    };
    assert!(!e3.is_no_event());

    let e4 = MwpmEvent::BlossomShatter {
        blossom: RegionIdx(0),
        in_parent: RegionIdx(1),
        in_child: RegionIdx(2),
    };
    assert!(!e4.is_no_event());
}

#[test]
fn flood_check_event_has_time() {
    let e = FloodCheckEvent::LookAtNode {
        node: NodeIdx(5),
        time: Wrapping(42),
    };
    assert_eq!(e.time(), Wrapping(42));

    let e2 = FloodCheckEvent::LookAtShrinkingRegion {
        region: RegionIdx(3),
        time: Wrapping(100),
    };
    assert_eq!(e2.time(), Wrapping(100));

    let e3 = FloodCheckEvent::LookAtSearchNode {
        node: SearchNodeIdx(7),
        time: Wrapping(200),
    };
    assert_eq!(e3.time(), Wrapping(200));

    let no = FloodCheckEvent::NoEvent;
    assert!(no.is_no_event());
    assert_eq!(no.time(), Wrapping(0));
}

#[test]
fn queued_event_tracker_basic() {
    let mut tracker = QueuedEventTracker::default();
    let mut queue: RadixHeapQueue<FloodCheckEvent> = RadixHeapQueue::new();

    // Set a desired event
    let event = FloodCheckEvent::LookAtNode {
        node: NodeIdx(0),
        time: Wrapping(10),
    };
    tracker.set_desired_event(event, &mut queue);
    assert!(tracker.has_desired_time);
    assert!(tracker.has_queued_time);
    assert_eq!(tracker.desired_time, Wrapping(10));

    // Dequeue and check decision
    let dequeued = queue.dequeue();
    let result = tracker.dequeue_decision(&dequeued, &mut queue, |t| {
        FloodCheckEvent::LookAtNode { node: NodeIdx(0), time: t }
    });
    assert!(result);
}

#[test]
fn queued_event_tracker_stale_event() {
    let mut tracker = QueuedEventTracker::default();
    let mut queue: RadixHeapQueue<FloodCheckEvent> = RadixHeapQueue::new();

    // Set a desired event
    let event = FloodCheckEvent::LookAtNode {
        node: NodeIdx(0),
        time: Wrapping(10),
    };
    tracker.set_desired_event(event, &mut queue);

    // Override with an earlier event
    let event2 = FloodCheckEvent::LookAtNode {
        node: NodeIdx(0),
        time: Wrapping(5),
    };
    tracker.set_desired_event(event2, &mut queue);

    // Dequeue the earlier event (time=5) first
    let dequeued = queue.dequeue();
    assert_eq!(dequeued.time(), Wrapping(5));
    let result = tracker.dequeue_decision(&dequeued, &mut queue, |t| {
        FloodCheckEvent::LookAtNode { node: NodeIdx(0), time: t }
    });
    assert!(result);

    // The stale event (time=10) should be rejected
    let stale = queue.dequeue();
    assert_eq!(stale.time(), Wrapping(10));
    let result2 = tracker.dequeue_decision(&stale, &mut queue, |t| {
        FloodCheckEvent::LookAtNode { node: NodeIdx(0), time: t }
    });
    assert!(!result2);
}

#[test]
fn queued_event_tracker_clear() {
    let mut tracker = QueuedEventTracker::default();
    let mut queue: RadixHeapQueue<FloodCheckEvent> = RadixHeapQueue::new();

    let event = FloodCheckEvent::LookAtNode {
        node: NodeIdx(0),
        time: Wrapping(10),
    };
    tracker.set_desired_event(event, &mut queue);
    tracker.clear();
    assert!(!tracker.has_desired_time);
    assert!(!tracker.has_queued_time);
}

#[test]
fn region_edge_and_match_construction() {
    let edge = CompressedEdge {
        loc_from: Some(NodeIdx(0)),
        loc_to: Some(NodeIdx(1)),
        obs_mask: 0,
    };
    let re = RegionEdge {
        region: RegionIdx(5),
        edge,
    };
    assert_eq!(re.region, RegionIdx(5));

    let m = Match {
        region: None,
        edge: CompressedEdge::empty(),
    };
    assert!(m.region.is_none());
}
