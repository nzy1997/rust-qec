#![no_main]

use libfuzzer_sys::fuzz_target;
use rstim::interactive_shot::{EditableShot, ExpansionLimits};

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        let _ = EditableShot::open(
            source,
            ExpansionLimits {
                max_operations: 256,
                max_noise_events: 128,
                max_measurements: 128,
                max_svg_nodes: 2_048,
                max_qubits: 64,
            },
            7,
        );
    }
});
