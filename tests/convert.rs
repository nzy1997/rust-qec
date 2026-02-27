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
