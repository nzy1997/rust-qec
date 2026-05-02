use std::num::Wrapping;
use crate::types::*;
use crate::util::radix_heap::HasTime;

#[derive(Debug, Clone, Copy)]
pub enum FloodCheckEvent {
    NoEvent,
    LookAtNode { node: NodeIdx, time: Wrapping<u32> },
    LookAtShrinkingRegion { region: RegionIdx, time: Wrapping<u32> },
    LookAtSearchNode { node: SearchNodeIdx, time: Wrapping<u32> },
}

impl HasTime for FloodCheckEvent {
    fn time(&self) -> Wrapping<u32> {
        match self {
            FloodCheckEvent::NoEvent => Wrapping(0),
            FloodCheckEvent::LookAtNode { time, .. } => *time,
            FloodCheckEvent::LookAtShrinkingRegion { time, .. } => *time,
            FloodCheckEvent::LookAtSearchNode { time, .. } => *time,
        }
    }

    fn no_event() -> Self {
        FloodCheckEvent::NoEvent
    }

    fn is_no_event(&self) -> bool {
        matches!(self, FloodCheckEvent::NoEvent)
    }
}
