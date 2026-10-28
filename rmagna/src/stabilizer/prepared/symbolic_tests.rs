use super::*;
use std::collections::BTreeMap;

#[test]
fn symbolic_prefixes_preserve_dependent_labels_padding_and_nonreal_sums() {
    let a = ScalarArithmetic::default();
    let locations: [Vec<usize>; 4] =
        std::array::from_fn(|j| vec![j % 2, 63 + j % 2, 128 + j, 1024 + j, 4093 + j]);
    let columns = locations
        .iter()
        .map(|v| BitVector::from_indices(4097, v.iter().copied()).unwrap())
        .collect();
    let mut upper = vec![BitVector::zeros(4).unwrap(); 4];
    for (i, j) in [(0, 1), (0, 3), (2, 3)] {
        upper[i].set(j, true).unwrap();
    }
    let original = AffineKet::new(
        BitVector::zeros(4097).unwrap(),
        columns,
        QuadraticForm::new(0, vec![1, 2, 3, 0], upper).unwrap(),
        ExactScalar::one(),
    )
    .unwrap();
    let contraction = PreparedAffineContraction::new(&original).unwrap();
    let mut nonreal = 0;
    let mut comparisons = 0;
    for width in [0, 1, 3, 63, 64, 65, 128, 129, 3073, 4097] {
        let symbolic = contraction.symbolic_boundary(width).unwrap();
        let expected_columns = locations
            .iter()
            .map(|v| {
                BitVector::from_indices(width, v.iter().copied().filter(|&j| j < width)).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(symbolic.columns(), expected_columns);
        assert_eq!(symbolic.coefficient(), &ExactScalar::one());
        let prepared = symbolic.prepare().unwrap();
        let mut truth = BTreeMap::<Vec<u64>, ExactScalar>::new();
        for x in 0..16 {
            let mut label = BitVector::zeros(width).unwrap();
            for (j, indices) in locations.iter().enumerate() {
                if x >> j & 1 != 0 {
                    for &i in indices.iter().filter(|&&i| i < width) {
                        label.set(i, !label.get(i).unwrap()).unwrap();
                    }
                }
            }
            let p = (x & 1)
                + 2 * ((x >> 1) & 1)
                + 3 * ((x >> 2) & 1)
                + 2 * (((x & 1) * ((x >> 1) & 1))
                    + ((x & 1) * ((x >> 3) & 1))
                    + (((x >> 2) & 1) * ((x >> 3) & 1)));
            let value = a.omega(2 * p as i64, 0).unwrap();
            let entry = truth
                .entry(label.words().to_vec())
                .or_insert_with(ExactScalar::zero);
            *entry = a.add(entry, &value).unwrap();
        }
        for (words, expected) in truth {
            let value = prepared
                .amplitude(&BitVector::from_words(width, words).unwrap(), &a)
                .unwrap();
            assert_eq!(value, expected, "prefix width {width}");
            comparisons += 1;
            nonreal += usize::from(!value.is_real());
        }
    }
    assert_eq!(comparisons, 63);
    assert!(nonreal >= 16);
    assert!(matches!(
        contraction.symbolic_boundary(4098),
        Err(StabilizerError::IncompatibleBoundaryMap)
    ));
}
