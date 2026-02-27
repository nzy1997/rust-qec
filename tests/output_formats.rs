use rstim::output::{OutputFormat, write_shots_01, write_shots_b8, write_shots_r8, write_shots_hits, write_shots_dets, write_shots_ptb64};
use rstim::sim::bit_table::BitTable;

#[test]
fn format_01_simple() {
    let mut table = BitTable::new(2, 3); // 2 bits, 3 shots
    table.set(0, 0, true);  // shot 0: bit 0 = 1
    table.set(1, 0, true);  // shot 0: bit 1 = 1
    table.set(0, 1, false); // shot 1: bit 0 = 0
    table.set(1, 1, true);  // shot 1: bit 1 = 1
    // shot 2: all false
    let mut buf = Vec::new();
    write_shots_01(&table, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "11\n01\n00\n");
}

#[test]
fn format_01_empty() {
    let table = BitTable::new(0, 3);
    let mut buf = Vec::new();
    write_shots_01(&table, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "\n\n\n");
}

#[test]
fn format_b8_simple() {
    let mut table = BitTable::new(10, 1);
    for i in 0..4 { table.set(i, 0, true); }
    let mut buf = Vec::new();
    write_shots_b8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![0x0F, 0x00]);
}

#[test]
fn format_b8_bit_order() {
    let mut table = BitTable::new(8, 1);
    table.set(0, 0, true);
    table.set(7, 0, true);
    let mut buf = Vec::new();
    write_shots_b8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![0x81]);
}

#[test]
fn format_r8_no_hits() {
    let table = BitTable::new(5, 1);
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![5]);
}

#[test]
fn format_r8_first_bit_set() {
    let mut table = BitTable::new(3, 1);
    table.set(0, 0, true);
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![0, 2]);
}

#[test]
fn format_r8_long_run() {
    let mut table = BitTable::new(301, 1);
    table.set(300, 0, true);
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![255, 45, 0]);
}

#[test]
fn format_r8_long_terminator_run() {
    // Bit 0 set, then 259 trailing zeros before end. Terminator run = 259 >= 255.
    let mut table = BitTable::new(260, 1);
    table.set(0, 0, true);
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    // [0 (run before bit 0), 255 (255 zeros no True), 4 (4 zeros then terminator)]
    assert_eq!(buf, vec![0, 255, 4]);
}

#[test]
fn format_r8_multiple_shots() {
    let mut table = BitTable::new(3, 2);
    table.set(0, 0, true);
    table.set(2, 1, true);
    let mut buf = Vec::new();
    write_shots_r8(&table, &mut buf).unwrap();
    // shot 0: bit 0 set -> [0, 2]  (0 before hit, 2 zeros then terminator)
    // shot 1: bit 2 set -> [2, 0]  (2 zeros before hit, 0 zeros then terminator)
    assert_eq!(buf, vec![0, 2, 2, 0]);
}

#[test]
fn format_b8_zero_bits() {
    let table = BitTable::new(0, 2);
    let mut buf = Vec::new();
    write_shots_b8(&table, &mut buf).unwrap();
    assert!(buf.is_empty());
}

#[test]
fn format_b8_multi_shot() {
    let mut table = BitTable::new(8, 2);
    table.set(0, 0, true);
    table.set(7, 1, true);
    let mut buf = Vec::new();
    write_shots_b8(&table, &mut buf).unwrap();
    assert_eq!(buf, vec![0x01, 0x80]);
}

#[test]
fn format_hits_no_hits() {
    let table = BitTable::new(5, 1);
    let mut buf = Vec::new();
    write_shots_hits(&table, &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "\n");
}

#[test]
fn format_dets_no_detections() {
    let dets = BitTable::new(3, 1);
    let obs = BitTable::new(2, 1);
    let mut buf = Vec::new();
    write_shots_dets(&dets, &obs, &mut buf).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "shot\n");
}

#[test]
fn format_hits_simple() {
    let mut table = BitTable::new(5, 2);
    table.set(1, 0, true);
    table.set(3, 0, true);
    let mut buf = Vec::new();
    write_shots_hits(&table, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "1,3\n\n");
}

#[test]
fn format_dets_simple() {
    let mut dets = BitTable::new(3, 2);
    dets.set(1, 0, true);
    let mut obs = BitTable::new(2, 2);
    obs.set(0, 1, true);
    let mut buf = Vec::new();
    write_shots_dets(&dets, &obs, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert_eq!(s, "shot D1\nshot L0\n");
}

#[test]
fn output_format_from_str() {
    assert_eq!(OutputFormat::from_str("01").unwrap(), OutputFormat::Format01);
    assert_eq!(OutputFormat::from_str("b8").unwrap(), OutputFormat::B8);
    assert_eq!(OutputFormat::from_str("r8").unwrap(), OutputFormat::R8);
    assert_eq!(OutputFormat::from_str("hits").unwrap(), OutputFormat::Hits);
    assert_eq!(OutputFormat::from_str("dets").unwrap(), OutputFormat::Dets);
    assert!(OutputFormat::from_str("unknown").is_err());
}

#[test]
fn ptb64_format_parses() {
    assert!(OutputFormat::from_str("ptb64").is_ok());
}

#[test]
fn ptb64_single_shot_single_bit() {
    let mut table = BitTable::new(1, 1);
    table.set(0, 0, true);
    let mut out = Vec::new();
    write_shots_ptb64(&table, &mut out).unwrap();
    assert_eq!(out.len(), 8);
    assert_eq!(u64::from_le_bytes(out[..8].try_into().unwrap()), 1u64);
}

#[test]
fn ptb64_two_bits_two_shots() {
    // shot0: bit0=1, bit1=0 / shot1: bit0=0, bit1=1
    // bit0 word: shot0=1, shot1=0 → 0b01 = 1
    // bit1 word: shot0=0, shot1=1 → 0b10 = 2
    let mut table = BitTable::new(2, 2);
    table.set(0, 0, true);
    table.set(1, 1, true);
    let mut out = Vec::new();
    write_shots_ptb64(&table, &mut out).unwrap();
    assert_eq!(out.len(), 16);
    let w0 = u64::from_le_bytes(out[0..8].try_into().unwrap());
    let w1 = u64::from_le_bytes(out[8..16].try_into().unwrap());
    assert_eq!(w0, 1u64);
    assert_eq!(w1, 2u64);
}

#[test]
fn ptb64_more_than_64_shots() {
    // 1 bit, 65 shots, all set
    // chunk0: 64 shots all set → u64::MAX
    // chunk1: 1 shot set → 1
    let mut table = BitTable::new(1, 65);
    for s in 0..65 { table.set(0, s, true); }
    let mut out = Vec::new();
    write_shots_ptb64(&table, &mut out).unwrap();
    assert_eq!(out.len(), 16);
    let w0 = u64::from_le_bytes(out[0..8].try_into().unwrap());
    let w1 = u64::from_le_bytes(out[8..16].try_into().unwrap());
    assert_eq!(w0, u64::MAX);
    assert_eq!(w1, 1u64);
}
