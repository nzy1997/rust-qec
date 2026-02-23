use rstim::sim::bit_table::BitTable;

#[test]
fn new_table_is_all_zeros() {
    let t = BitTable::new(4, 128);
    for r in 0..4 {
        for c in 0..128 {
            assert_eq!(t.get(r, c), false);
        }
    }
}

#[test]
fn set_and_get() {
    let mut t = BitTable::new(3, 200);
    t.set(1, 77, true);
    assert_eq!(t.get(1, 77), true);
    assert_eq!(t.get(1, 78), false);
    assert_eq!(t.get(0, 77), false);
    t.set(1, 77, false);
    assert_eq!(t.get(1, 77), false);
}

#[test]
fn xor_row() {
    let mut t = BitTable::new(3, 128);
    t.set(0, 10, true);
    t.set(0, 50, true);
    t.set(1, 50, true);
    t.set(1, 90, true);
    t.xor_row(0, 1);
    assert_eq!(t.get(0, 10), true);
    assert_eq!(t.get(0, 50), false);
    assert_eq!(t.get(0, 90), true);
}

#[test]
fn swap_rows() {
    let mut t = BitTable::new(2, 64);
    t.set(0, 0, true);
    t.set(1, 63, true);
    t.swap_rows(0, 1);
    assert_eq!(t.get(0, 0), false);
    assert_eq!(t.get(0, 63), true);
    assert_eq!(t.get(1, 0), true);
    assert_eq!(t.get(1, 63), false);
}

#[test]
fn clear_row() {
    let mut t = BitTable::new(2, 128);
    t.set(0, 5, true);
    t.set(0, 100, true);
    t.clear_row(0);
    assert_eq!(t.get(0, 5), false);
    assert_eq!(t.get(0, 100), false);
}

#[test]
fn num_minor_and_major() {
    let t = BitTable::new(5, 130);
    assert_eq!(t.num_major(), 5);
    assert_eq!(t.num_minor(), 130);
}
