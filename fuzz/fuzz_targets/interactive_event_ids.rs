#![no_main]

use libfuzzer_sys::fuzz_target;
use rstim::interactive_shot::{NoiseEventId, NoiseSiteId};

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = std::str::from_utf8(data) {
        if let Ok(id) = NoiseEventId::decode(value) {
            assert_eq!(NoiseEventId::decode(&id.encode()).unwrap(), id);
        }
        if let Ok(id) = NoiseSiteId::decode(value) {
            assert_eq!(NoiseSiteId::decode(&id.encode()).unwrap(), id);
        }
    }
});
