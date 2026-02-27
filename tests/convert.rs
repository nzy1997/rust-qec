use rstim::cli::run_convert;
use rstim::output::{read_shots_01, read_shots_b8, read_shots_r8, read_shots_hits, read_shots_ptb64,
                    write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits, write_shots_ptb64};
use rstim::sim::bit_table::BitTable;

fn make_table(bits: usize, shots: usize, pattern: impl Fn(usize, usize) -> bool) -> BitTable {
    let mut t = BitTable::new(bits, shots);
    for b in 0..bits {
        for s in 0..shots {
            if pattern(b, s) { t.set(b, s, true); }
        }
    }
    t
}

fn tables_equal(a: &BitTable, b: &BitTable) -> bool {
    if a.num_major() != b.num_major() || a.num_minor() != b.num_minor() { return false; }
    for i in 0..a.num_major() {
        for j in 0..a.num_minor() {
            if a.get(i, j) != b.get(i, j) { return false; }
        }
    }
    true
}

#[test]
fn roundtrip_01() {
    let orig = make_table(4, 3, |b, s| (b + s) % 2 == 0);
    let mut buf = Vec::new();
    write_shots_01(&orig, &mut buf).unwrap();
    let recovered = read_shots_01(&buf, 4).unwrap();
    assert!(tables_equal(&orig, &recovered));
}

#[test]
fn roundtrip_b8() {
    let orig = make_table(5, 2, |b, s| b == s);
    let mut buf = Vec::new();
    write_shots_b8(&orig, &mut buf).unwrap();
    let recovered = read_shots_b8(&buf, 5).unwrap();
    assert!(tables_equal(&orig, &recovered));
}

#[test]
fn roundtrip_r8() {
    let orig = make_table(6, 3, |b, s| b % 3 == s);
    let mut buf = Vec::new();
    write_shots_r8(&orig, &mut buf).unwrap();
    let recovered = read_shots_r8(&buf, 6).unwrap();
    assert!(tables_equal(&orig, &recovered));
}

#[test]
fn roundtrip_hits() {
    let orig = make_table(4, 2, |b, s| b == 1 && s == 0);
    let mut buf = Vec::new();
    write_shots_hits(&orig, &mut buf).unwrap();
    let recovered = read_shots_hits(&buf, 4).unwrap();
    assert!(tables_equal(&orig, &recovered));
}

#[test]
fn roundtrip_ptb64() {
    let orig = make_table(3, 70, |b, s| (b * 7 + s) % 5 == 0);
    let mut buf = Vec::new();
    write_shots_ptb64(&orig, &mut buf).unwrap();
    let recovered = read_shots_ptb64(&buf, 3, 70).unwrap();
    assert!(tables_equal(&orig, &recovered));
}

#[test]
fn convert_01_to_b8() {
    // shot0: bits 0,2 set → byte = 0b0101 = 5
    // shot1: bits 1,3 set → byte = 0b1010 = 10
    let input = b"1010\n0101\n";
    let mut out = Vec::new();
    run_convert(input, "01", "b8", Some(4), None, None, &mut out).unwrap();
    assert_eq!(out, vec![5u8, 10u8]);
}

#[test]
fn convert_b8_to_01() {
    let input = vec![5u8, 10u8]; // same as above
    let mut out = Vec::new();
    run_convert(&input, "b8", "01", Some(4), None, None, &mut out).unwrap();
    assert_eq!(out, b"1010\n0101\n");
}

#[test]
fn convert_requires_bits_or_circuit() {
    let input = b"1010\n";
    let mut out = Vec::new();
    let result = run_convert(input, "01", "b8", None, None, None, &mut out);
    assert!(result.is_err());
}
