use rmagna::circuit::{BitAtom, BitExpr, Gate, Operation, ParseOptions, parse_circuit};
use std::path::PathBuf;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn classical_second_cz_endpoint_keeps_the_original_record_and_quantum_wire() {
    let root = std::env::temp_dir().join(format!(
        "rmagna-coverage-cz-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    let path = fixture.0.join("input.magna");
    let source = "M 0\nCZ 1 rec[-1]\n";
    std::fs::write(&path, source).unwrap();
    let loaded = std::fs::read_to_string(&path).unwrap();
    let circuit = parse_circuit(&loaded, &ParseOptions::default()).unwrap();
    assert_eq!(circuit.to_text(), source);
    assert_eq!(circuit.num_records(), 1);
    match &circuit.operations().last().unwrap().operation {
        Operation::Unitary {
            gate,
            qubits,
            control,
        } => {
            assert_eq!(gate, &Gate::Z);
            assert_eq!(qubits, &vec![1]);
            assert_eq!(control, &Some(BitExpr::atom(BitAtom::Record(0))));
        }
        operation => panic!("expected the original classically controlled Z, found {operation:?}"),
    }
}
