#![no_main]

use libfuzzer_sys::fuzz_target;
use rstim::qasm::{
    Backend, ExportOptions, QasmCircuit, QasmLimits, RejectIncludes, parse_and_check, to_qasm,
};

fn limits() -> QasmLimits {
    QasmLimits {
        max_source_bytes: 4096,
        max_include_files: 4,
        max_include_depth: 4,
        max_gate_depth: 16,
        max_expr_depth: 16,
        max_tokens: 2048,
        max_operations: 256,
        max_qubits: 4,
        max_classical_bits: 16,
        max_output_bytes: 16_384,
    }
}

fn check(circuit: QasmCircuit, limits: &QasmLimits) {
    if let Ok(json) = circuit.to_json(limits) {
        let restored = QasmCircuit::from_json(&json, limits).expect("own document must validate");
        assert_eq!(circuit, restored);
    }
    if let Ok(output) = to_qasm(&circuit, &ExportOptions::default(), limits) {
        let export_limits = QasmLimits {
            max_source_bytes: limits.max_output_bytes,
            max_tokens: 16_384,
            max_operations: 2048,
            ..limits.clone()
        };
        // Primitive expansion can increase operation count. Apply an independent
        // bounded reimport budget, rather than requiring byte-identical text.
        let restored = parse_and_check(&output.text, "<export>", &RejectIncludes, &export_limits)
            .expect("portable export must parse within its expansion budget");
        assert_eq!(circuit.quantum_registers(), restored.quantum_registers());
        assert_eq!(
            circuit.classical_registers(),
            restored.classical_registers()
        );
        for backend in [Backend::Stabilizer, Backend::NearClifford] {
            if let (Ok(before), Ok(after)) = (
                circuit.lower(backend, &export_limits),
                restored.lower(backend, &export_limits),
            ) {
                assert_eq!(before.num_qubits, after.num_qubits);
                assert_eq!(before.classical_outputs, after.classical_outputs);
                assert_eq!(
                    before.measurement_events.len(),
                    after.measurement_events.len()
                );
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let limits = limits();
    if data.len() > limits.max_source_bytes {
        return;
    }
    // The public source API accepts UTF-8; malformed byte inputs terminate at
    // this boundary instead of being normalized into a different program.
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(circuit) = parse_and_check(source, "<fuzz>", &RejectIncludes, &limits) {
        check(circuit, &limits);
    }
});
