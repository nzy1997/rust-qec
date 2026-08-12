#![no_main]

use libfuzzer_sys::fuzz_target;
use rstim::qp101::{Qp101Document, Qp101Operation};
use rstim::qp101_svg::render_svg;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let document = Qp101Document {
            standard: "QP101-ZY".to_string(),
            version: "0.1.0".to_string(),
            num_qubits: 1,
            operations: vec![Qp101Operation::Annotation {
                kind: "fuzz".to_string(),
                text: text.to_string(),
                annotations: Vec::new(),
            }],
            metadata: None,
            extensions: None,
        };
        let svg = render_svg(&document).unwrap();
        assert!(!svg.contains("<script>"));
        assert!(!svg.contains("</text><"));
    }
});
