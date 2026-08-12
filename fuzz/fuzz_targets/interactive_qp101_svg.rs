#![no_main]

use libfuzzer_sys::fuzz_target;
use rstim::qp101::Qp101Document;
use rstim::qp101_svg::render_svg;

fuzz_target!(|data: &[u8]| {
    if let Ok(document) = serde_json::from_slice::<Qp101Document>(data) {
        if let Ok(svg) = render_svg(&document) {
            assert!(svg.is_char_boundary(svg.len()));
            assert!(svg.starts_with("<svg"));
        }
    }
});
