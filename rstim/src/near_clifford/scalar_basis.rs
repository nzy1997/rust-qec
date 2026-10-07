// Scalar virtual-frame programs; original gate vectors remain authoritative.
// Preserve the original gate vectors for packet execution and dense test oracles.
use super::*;

#[derive(Clone, Copy, Debug)]
enum StarKind {
    FanoutCx,
    FaninCx,
    Cz,
}

#[derive(Clone, Copy, Debug)]
struct WordMask {
    word: usize,
    mask: u64,
}

#[derive(Clone, Debug)]
enum ScalarStep {
    Gates {
        start: usize,
        end: usize,
    },
    Star {
        kind: StarKind,
        pivot: usize,
        masks: Vec<WordMask>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct ScalarBasisProgram {
    steps: Vec<ScalarStep>,
}

// Two possible orientations for the first gate. Choose the longest contiguous
// run, never move a gate through an H, S, or differently oriented star.
fn star(gates: &[BasisGate], start: usize) -> Option<(StarKind, usize, usize)> {
    let candidates = match gates[start] {
        BasisGate::CX(a, b) if a != b => [(StarKind::FanoutCx, a), (StarKind::FaninCx, b)],
        BasisGate::CZ(a, b) if a != b => [(StarKind::Cz, a), (StarKind::Cz, b)],
        _ => return None,
    };
    let mut result = None;
    let mut best = start + 2; // Three gates minimum; short programs keep scalar gates.
    for (kind, pivot) in candidates {
        let mut end = start;
        while end < gates.len() && target(kind, pivot, gates[end]).is_some() {
            end += 1;
        }
        if end > best {
            result = Some((kind, pivot, end));
            best = end;
        }
    }
    result
}

fn target(kind: StarKind, pivot: usize, gate: BasisGate) -> Option<usize> {
    match (kind, gate) {
        (StarKind::FanoutCx, BasisGate::CX(a, b)) if a == pivot && b != pivot => Some(b),
        (StarKind::FaninCx, BasisGate::CX(a, b)) if b == pivot && a != pivot => Some(a),
        (StarKind::Cz, BasisGate::CZ(a, b)) if a == pivot && b != pivot => Some(b),
        (StarKind::Cz, BasisGate::CZ(a, b)) if b == pivot && a != pivot => Some(a),
        _ => None,
    }
}

fn masks_for(gates: &[BasisGate], kind: StarKind, pivot: usize, n: usize) -> Option<[u64; 64]> {
    // The executor already caps physical width at 4096. Fixed scratch avoids
    // uncharged transient heap growth, even when compiling many tiny programs.
    if n > 4096 || pivot >= n {
        return None;
    }
    let mut masks = [0u64; 64];
    for &gate in gates {
        let q = target(kind, pivot, gate)?;
        if q >= n {
            return None;
        }
        masks[q / 64] ^= 1u64 << (q % 64); // Repeated targets cancel exactly.
    }
    Some(masks)
}

impl ScalarBasisProgram {
    fn build(gates: &[BasisGate], n: usize, budget: usize) -> Option<(Self, usize)> {
        let mut steps = 0usize;
        let mut words = 0usize;
        let mut has_star = false;
        let mut cursor = 0;
        while cursor < gates.len() {
            if let Some((kind, pivot, end)) = star(gates, cursor) {
                has_star = true;
                steps = steps.checked_add(1)?;
                words = words.checked_add(
                    masks_for(&gates[cursor..end], kind, pivot, n)?
                        .iter()
                        .filter(|&&mask| mask != 0)
                        .count(),
                )?;
                cursor = end;
            } else {
                steps = steps.checked_add(1)?;
                cursor += 1;
                while cursor < gates.len() && star(gates, cursor).is_none() {
                    cursor += 1;
                }
            }
        }
        if !has_star {
            return None;
        }
        let requested = steps
            .checked_mul(size_of::<ScalarStep>())?
            .checked_add(words.checked_mul(size_of::<WordMask>())?)?;
        if requested > budget {
            return None;
        }
        let mut program = Self { steps: Vec::new() };
        program.steps.try_reserve_exact(steps).ok()?;
        let mut used = program
            .steps
            .capacity()
            .checked_mul(size_of::<ScalarStep>())?;
        if used > budget {
            return None;
        }
        cursor = 0;
        while cursor < gates.len() {
            if let Some((kind, pivot, end)) = star(gates, cursor) {
                let packed = masks_for(&gates[cursor..end], kind, pivot, n)?;
                let count = packed.iter().filter(|&&mask| mask != 0).count();
                let mut masks = Vec::new();
                masks.try_reserve_exact(count).ok()?;
                used = used.checked_add(masks.capacity().checked_mul(size_of::<WordMask>())?)?;
                if used > budget {
                    return None;
                }
                for (word, &mask) in packed.iter().enumerate() {
                    if mask != 0 {
                        masks.push(WordMask { word, mask });
                    }
                }
                program.steps.push(ScalarStep::Star { kind, pivot, masks });
                cursor = end;
            } else {
                let start = cursor;
                cursor += 1;
                while cursor < gates.len() && star(gates, cursor).is_none() {
                    cursor += 1;
                }
                program.steps.push(ScalarStep::Gates { start, end: cursor });
            }
        }
        Some((program, used))
    }

    #[inline]
    pub(super) fn apply(&self, gates: &[BasisGate], x: &mut [u64], z: &mut [u64]) {
        for step in &self.steps {
            match step {
                ScalarStep::Gates { start, end } => {
                    for &gate in &gates[*start..*end] {
                        gate.conjugate(x, z);
                    }
                }
                ScalarStep::Star { kind, pivot, masks } => {
                    let word = pivot / 64;
                    let bit = pivot % 64;
                    match kind {
                        StarKind::FanoutCx => {
                            let broadcast = 0u64.wrapping_sub((x[word] >> bit) & 1);
                            let mut parity = 0u64;
                            for &WordMask { word, mask } in masks {
                                parity ^= z[word] & mask;
                                x[word] ^= broadcast & mask;
                            }
                            z[word] ^= u64::from(parity.count_ones() & 1) << bit;
                        }
                        StarKind::FaninCx => {
                            let broadcast = 0u64.wrapping_sub((z[word] >> bit) & 1);
                            let mut parity = 0u64;
                            for &WordMask { word, mask } in masks {
                                parity ^= x[word] & mask;
                                z[word] ^= broadcast & mask;
                            }
                            x[word] ^= u64::from(parity.count_ones() & 1) << bit;
                        }
                        StarKind::Cz => {
                            let broadcast = 0u64.wrapping_sub((x[word] >> bit) & 1);
                            let mut parity = 0u64;
                            for &WordMask { word, mask } in masks {
                                parity ^= x[word] & mask;
                                z[word] ^= broadcast & mask;
                            }
                            z[word] ^= u64::from(parity.count_ones() & 1) << bit;
                        }
                    }
                }
            }
        }
    }
}

// Optional metadata: reject without invalidating the original plan. Each
// program's actual capacities, plus the outer table, consume the same remaining
// 64 MiB plan budget as any later random-run metadata. No per-shot allocation.
pub(super) fn build_scalar_basis(
    operations: &[PlanOp],
    n: usize,
    budget: usize,
) -> Option<(Vec<Option<ScalarBasisProgram>>, usize)> {
    let header = size_of::<Vec<Option<ScalarBasisProgram>>>();
    let requested = operations
        .len()
        .checked_mul(size_of::<Option<ScalarBasisProgram>>())?
        .checked_add(header)?;
    if requested > budget {
        return None;
    }
    let mut programs = Vec::new();
    programs.try_reserve_exact(operations.len()).ok()?;
    let mut used = programs
        .capacity()
        .checked_mul(size_of::<Option<ScalarBasisProgram>>())?
        .checked_add(header)?;
    if used > budget {
        return None;
    }
    let mut any = false;
    for op in operations {
        let gates = match op {
            PlanOp::Basis(gates) => Some(gates.as_slice()),
            PlanOp::Measure(m) => Some(m.basis.as_slice()),
            _ => None,
        };
        let program = gates.and_then(|gates| ScalarBasisProgram::build(gates, n, budget - used));
        if let Some((program, bytes)) = program {
            used = used.checked_add(bytes)?;
            any = true;
            programs.push(Some(program));
        } else {
            programs.push(None);
        }
    }
    any.then_some((programs, used))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    // Independent old sequential Boolean semantics: no mask formulas and no
    // call to the current production conjugate or packet implementation.
    fn old_oracle(gates: &[BasisGate], x: &mut [bool], z: &mut [bool]) {
        for &gate in gates {
            match gate {
                BasisGate::H(q) => std::mem::swap(&mut x[q], &mut z[q]),
                BasisGate::S(q) => {
                    if x[q] {
                        z[q] ^= true;
                    }
                }
                BasisGate::CX(a, b) => {
                    if x[a] {
                        x[b] ^= true;
                    }
                    if z[b] {
                        z[a] ^= true;
                    }
                }
                BasisGate::CZ(a, b) => {
                    if x[a] {
                        z[b] ^= true;
                    }
                    if x[b] {
                        z[a] ^= true;
                    }
                }
            }
        }
    }
    fn compare(gates: &[BasisGate], n: usize, x: Vec<bool>, z: Vec<bool>) {
        let pack = |bits: &[bool]| {
            let mut result = vec![0u64; n.div_ceil(64)];
            for (q, &bit) in bits.iter().enumerate() {
                if bit {
                    result[q / 64] |= 1 << (q % 64);
                }
            }
            result
        };
        let (mut packed_x, mut packed_z) = (pack(&x), pack(&z));
        let (mut expected_x, mut expected_z) = (x, z);
        old_oracle(gates, &mut expected_x, &mut expected_z);
        let (program, _) = ScalarBasisProgram::build(gates, n, 1 << 20).expect("star admitted");
        program.apply(gates, &mut packed_x, &mut packed_z);
        assert_eq!(packed_x, pack(&expected_x));
        assert_eq!(packed_z, pack(&expected_z));
    }

    #[test]
    fn stars_match_old_boolean_oracle_at_word_boundaries_and_maximum_width() {
        let mut rng = StdRng::seed_from_u64(2026100709);
        for n in [65usize, 129, 4096] {
            for pivot in [0, 1, 62, 63, 64, n - 1] {
                if pivot >= n {
                    continue;
                }
                let targets = (0..n).filter(|&q| q != pivot).collect::<Vec<_>>();
                for kind in [StarKind::FanoutCx, StarKind::FaninCx, StarKind::Cz] {
                    for reverse in [false, true] {
                        let mut gates = Vec::new();
                        // Noncommuting gates delimit stars: compilation must not
                        // shuffle their position or merge across the delimiter.
                        gates.push(BasisGate::S(pivot));
                        for (i, &q) in targets.iter().enumerate() {
                            gates.push(match kind {
                                StarKind::FanoutCx => BasisGate::CX(pivot, q),
                                StarKind::FaninCx => BasisGate::CX(q, pivot),
                                StarKind::Cz if i % 2 == 0 => BasisGate::CZ(pivot, q),
                                StarKind::Cz => BasisGate::CZ(q, pivot),
                            });
                        }
                        if reverse {
                            gates.reverse();
                        }
                        gates.push(BasisGate::H(pivot));
                        for _ in 0..16 {
                            compare(
                                &gates,
                                n,
                                (0..n).map(|_| rng.r#gen()).collect(),
                                (0..n).map(|_| rng.r#gen()).collect(),
                            );
                        }
                        // Every pivot X/Z combination, broad untouched-bit
                        // sentinels and all-zero/all-one target inputs.
                        for input in 0..16 {
                            let mut x = vec![input & 4 != 0; n];
                            let mut z = vec![input & 8 != 0; n];
                            x[pivot] = input & 1 != 0;
                            z[pivot] = input & 2 != 0;
                            compare(&gates, n, x, z);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn mixed_stars_duplicate_cancellation_and_budget_fallback_preserve_original_gates() {
        let gates = vec![
            BasisGate::CX(63, 0),
            BasisGate::CX(63, 64),
            BasisGate::CX(63, 128),
            BasisGate::CX(63, 64),
            BasisGate::S(63),
            BasisGate::H(128),
            BasisGate::CX(0, 128),
            BasisGate::CX(64, 128),
            BasisGate::CX(63, 128),
            BasisGate::CZ(128, 0),
            BasisGate::CZ(63, 128),
            BasisGate::CZ(128, 64),
            BasisGate::CZ(128, 64),
            BasisGate::S(64),
        ];
        let mut rng = StdRng::seed_from_u64(719);
        for _ in 0..64 {
            compare(
                &gates,
                129,
                (0..129).map(|_| rng.r#gen()).collect(),
                (0..129).map(|_| rng.r#gen()).collect(),
            );
        }
        assert!(ScalarBasisProgram::build(&gates, 129, 0).is_none());
        let (_, bytes) = ScalarBasisProgram::build(&gates, 129, 1 << 20).unwrap();
        assert!(ScalarBasisProgram::build(&gates, 129, bytes - 1).is_none());
        let all_cancelled = vec![BasisGate::CZ(63, 64); 4];
        compare(&all_cancelled, 129, vec![true; 129], vec![false; 129]);
        let operations = vec![
            PlanOp::Basis(gates.clone()),
            PlanOp::Basis(vec![BasisGate::H(0)]),
        ];
        assert!(build_scalar_basis(&operations, 129, 0).is_none());
        let (programs, total) = build_scalar_basis(&operations, 129, 1 << 20).unwrap();
        assert!(programs[0].is_some());
        assert!(programs[1].is_none());
        assert!(total <= 1 << 20);
        let (fallback, _) = build_scalar_basis(&operations, 129, total - bytes).unwrap_or_default();
        assert!(fallback.iter().all(Option::is_none));
    }
}
