// SPDX-License-Identifier: MPL-2.0
// Case schedule adapted from itpyi/magna tests/native/gf2_kernel.cpp at
// 56847478c840395735146ef1e1ccdc3d8aedc10f, Mozilla Public License 2.0.
// The fixture contains C++ inputs, not expected packed-kernel outputs.
use rmagna::gf2::{BitVector, Gf2Error, parities, transpose_columns, xor_rows_prefix, xors};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    seed: u64,
    first_word: u64,
    upstream_commit: String,
    cases: Vec<Case>,
    next_word: u64,
    packed_cases: usize,
    original_packed_checks: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    n: usize,
    m: usize,
    density: u64,
    rows: Vec<String>,
    value: String,
    mask: String,
    initials: Vec<String>,
    next_word: u64,
}

#[path = "support/original_gf2.rs"]
mod original_gf2;
use original_gf2::{Mt19937, bytes, equals_bytes, pack};

#[test]
fn original_packed_byte_oracles_follow_pinned_stream() {
    let raw = include_bytes!("fixtures/original_packed_inputs.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "48a359072f42ffc1f656a3db18ee1ac4b0bad096077eb2b300ffdc603d694cce"
    );
    let inputs: Inputs = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        inputs.upstream_commit,
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(inputs.seed, 2026100409);
    assert_eq!(inputs.first_word, 890827357352823111);
    assert_eq!(inputs.next_word, 11929231392562902635);
    assert_eq!(inputs.original_packed_checks, 576873);
    assert_eq!(inputs.packed_cases, 162);
    assert_eq!(inputs.cases.len(), 162);
    let mut rng = Mt19937::new(inputs.seed);
    assert_eq!(rng.clone().next(), inputs.first_word);
    let mut cases = inputs.cases.iter();
    for n in [0, 1, 63, 64, 65, 127, 128, 129, 257] {
        for m in [0, 1, 3, 4, 7, 63, 64, 65, 137] {
            for density in [2, 17] {
                let c = cases.next().unwrap();
                assert_eq!((c.n, c.m, c.density), (n, m, density));
                let mut a = (0..m).map(|_| rng.row(n, density)).collect::<Vec<_>>();
                if m > 2 {
                    a[0].fill(0);
                    a[m - 1] = a[m - 2].clone();
                }
                assert_eq!(c.rows.iter().map(|r| bytes(r, n)).collect::<Vec<_>>(), a);
                let value = rng.row(n + 3, 2);
                let mask = rng.row(m, 2);
                assert_eq!(bytes(&c.value, n + 3), value);
                assert_eq!(bytes(&c.mask, m), mask);
                let rows = a.iter().map(|r| pack(r)).collect::<Vec<_>>();
                let snapshot = rows.clone();
                let v = pack(&value);
                for limit in [0, 1, 63, 64, 65, 128, n, n + 91] {
                    equals_bytes(&v.low(limit).unwrap(), &value[..limit.min(value.len())]);
                }
                let expected = a
                    .iter()
                    .map(|r| r.iter().zip(&value).fold(0, |s, (a, b)| s ^ (a * b)))
                    .collect::<Vec<_>>();
                equals_bytes(&parities(&rows, &v).unwrap(), &expected);
                equals_bytes(
                    &parities(&rows, &BitVector::zeros(0).unwrap()).unwrap(),
                    &vec![0; m],
                );
                let mut expected = vec![0; n];
                for i in 0..m {
                    if mask[i] != 0 {
                        for j in 0..n {
                            expected[j] ^= a[i][j];
                        }
                    }
                }
                equals_bytes(&xors(n, &rows, &pack(&mask)).unwrap(), &expected);
                let mut extended = mask.clone();
                extended.resize(m + 67, 0);
                extended[m + 66] = 1;
                let selector = pack(&extended);
                assert_eq!(c.initials.len(), 7);
                for (k, limit) in [0, 1, 63, 64, 65, m, m + 91].into_iter().enumerate() {
                    let initial = rng.row(n + 7, 2);
                    assert_eq!(bytes(&c.initials[k], n + 7), initial);
                    let mut expected = initial.clone();
                    for i in 0..m.min(limit) {
                        if extended[i] != 0 {
                            for j in 0..n {
                                expected[j] ^= a[i][j];
                            }
                        }
                    }
                    let mut destination = pack(&initial);
                    xor_rows_prefix(&mut destination, &rows, &selector, limit).unwrap();
                    equals_bytes(&destination, &expected);
                }
                let transposed = transpose_columns(n, &rows).unwrap();
                assert_eq!(transposed.len(), n);
                for (i, row) in transposed.iter().enumerate() {
                    equals_bytes(row, &(0..m).map(|j| a[j][i]).collect::<Vec<_>>());
                }
                assert_eq!(rows, snapshot);
                equals_bytes(&v, &value);
                equals_bytes(&selector, &extended);
                assert_eq!(rng.clone().next(), c.next_word);
            }
        }
    }
    assert!(cases.next().is_none());
    assert_eq!(rng.next(), inputs.next_word);
}

#[test]
fn checked_packed_operations_preserve_dimensions_padding_and_atomicity() {
    let one = BitVector::from_indices(1, [0]).unwrap();
    assert_eq!(one.low(usize::MAX).unwrap(), one);
    assert_eq!(
        BitVector::zeros(0).unwrap().low(usize::MAX).unwrap(),
        BitVector::zeros(0).unwrap()
    );
    let value = BitVector::from_indices(129, [0, 63, 64, 65, 127, 128]).unwrap();
    for limit in [0, 1, 63, 64, 65, 127, 128, 129, usize::MAX] {
        let prefix = value.low(limit).unwrap();
        assert_eq!(
            prefix,
            BitVector::from_indices(limit.min(129), value.ones().filter(|&i| i < limit)).unwrap()
        );
        assert!(prefix.words().len() <= value.words().len());
    }
    assert_eq!(
        BitVector::from_words(65, vec![0, 2]),
        Err(Gf2Error::NonzeroPadding)
    );
    assert_eq!(
        BitVector::from_words(65, vec![0]),
        Err(Gf2Error::InvalidWordCount)
    );
    let invalid = BitVector::from_indices(65, [64]).unwrap();
    assert_eq!(
        xors(1, std::slice::from_ref(&one), &invalid),
        Err(Gf2Error::IndexOutOfBounds {
            index: 64,
            width: 1
        })
    );
    assert_eq!(
        xors(1, &[], &invalid),
        Err(Gf2Error::IndexOutOfBounds {
            index: 64,
            width: 0
        })
    );
    assert_eq!(
        xors(
            1,
            std::slice::from_ref(&one),
            &BitVector::zeros(65).unwrap()
        )
        .unwrap(),
        BitVector::zeros(1).unwrap()
    );
    assert_eq!(
        xors(65, &[], &BitVector::zeros(0).unwrap()).unwrap(),
        BitVector::zeros(65).unwrap()
    );
    let mut destination = BitVector::from_indices(72, [0, 64, 65, 71]).unwrap();
    let snapshot = destination.clone();
    let row = BitVector::from_indices(65, [0, 64]).unwrap();
    xor_rows_prefix(
        &mut destination,
        std::slice::from_ref(&row),
        &invalid,
        usize::MAX,
    )
    .unwrap();
    assert_eq!(destination, snapshot); // Out-of-list selector is clipped here.
    xor_rows_prefix(
        &mut destination,
        std::slice::from_ref(&row),
        &one,
        usize::MAX,
    )
    .unwrap();
    assert_eq!(destination, BitVector::from_indices(72, [65, 71]).unwrap());
    let wide = BitVector::zeros(73).unwrap();
    let rows = [row.clone(), wide.clone()];
    let both = BitVector::from_indices(2, [0, 1]).unwrap();
    let snapshot = destination.clone();
    assert_eq!(
        xor_rows_prefix(&mut destination, &rows, &both, usize::MAX),
        Err(Gf2Error::WidthMismatch {
            expected: 72,
            actual: 73
        })
    );
    assert_eq!(destination, snapshot);
    // An unselected bad width is harmless to the prefix operation, while the
    // rectangular operation validates all rows even for an empty selection.
    xor_rows_prefix(&mut destination, &rows, &both, 1).unwrap();
    assert_eq!(
        destination,
        BitVector::from_indices(72, [0, 64, 65, 71]).unwrap()
    );
    assert_eq!(
        xors(65, &rows, &BitVector::zeros(0).unwrap()),
        Err(Gf2Error::WidthMismatch {
            expected: 65,
            actual: 73
        })
    );
    assert_eq!(
        row.dot(&one),
        Err(Gf2Error::WidthMismatch {
            expected: 65,
            actual: 1
        })
    );
    let mut strict = row.clone();
    assert_eq!(
        strict.xor_assign(&one),
        Err(Gf2Error::WidthMismatch {
            expected: 65,
            actual: 1
        })
    );
    assert_eq!(strict, row);
    let ragged = [one.clone(), row.clone(), wide];
    assert_eq!(
        parities(&ragged, &row).unwrap(),
        BitVector::from_indices(3, [0]).unwrap()
    );
    assert_eq!(
        transpose_columns(0, &[one]),
        Err(Gf2Error::WidthMismatch {
            expected: 0,
            actual: 1
        })
    );
    assert_eq!(
        transpose_columns(usize::MAX, &[row]),
        Err(Gf2Error::WidthMismatch {
            expected: usize::MAX,
            actual: 65
        })
    );
    assert_eq!(
        transpose_columns(usize::MAX, &[]),
        Err(Gf2Error::AllocationFailed)
    );
    assert_eq!(
        transpose_columns(0, &[BitVector::zeros(0).unwrap()]).unwrap(),
        Vec::<BitVector>::new()
    );
    assert_eq!(
        transpose_columns(3, &[]).unwrap(),
        vec![BitVector::zeros(0).unwrap(); 3]
    );
}
