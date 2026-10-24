//! Certify a bounded external phase catalogue without compiling quantum tasks.
use rmagna::catalogue::{CatalogueLimits, PhaseCatalogue};
use rmagna::gf2::BitVector;
use rmagna::phase::{CanonicalPhaseCore, ParityPhase, PhaseCoreLimits};
use rmagna::scalar::ScalarArithmetic;
use sha2::{Digest, Sha256};
use std::io::Read;
// Independent finite-table inversion for the optional external-core audit.
// The production normalizer works directly with parity axes instead.
fn phase_from_table(
    dimension: usize,
    table: &[u8],
) -> Result<ParityPhase, Box<dyn std::error::Error>> {
    let mut polynomial = table.to_vec();
    for bit in 0..dimension {
        for word in 0..table.len() {
            if word & (1 << bit) != 0 {
                polynomial[word] = (polynomial[word] + 8 - polynomial[word ^ (1 << bit)]) & 7;
            }
        }
    }
    let mut terms = Vec::new();
    for (mask, &coefficient) in polynomial.iter().enumerate().skip(1) {
        if coefficient == 0 {
            continue;
        }
        let degree = mask.count_ones();
        if degree > 3 || u32::from(coefficient) % (1 << (degree - 1)) != 0 {
            return Err("catalogue phase table is not a third-level polynomial".into());
        }
        let scale = coefficient >> (degree - 1);
        let mut sub = mask;
        while sub != 0 {
            terms.push((
                BitVector::from_indices(dimension, (0..dimension).filter(|j| sub >> j & 1 != 0))?,
                if sub.count_ones() % 2 == 1 {
                    scale
                } else {
                    (8 - scale) & 7
                },
            ));
            sub = (sub - 1) & mask;
        }
    }
    Ok(ParityPhase::new(dimension, polynomial[0], terms)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: validate_phase_catalogue CATALOGUE_JSON [--check-canonical-cores]")?;
    let check_cores = match (args.next(), args.next()) {
        (None, None) => false,
        (Some(flag), None) if flag == "--check-canonical-cores" => true,
        _ => {
            return Err(
                "usage: validate_phase_catalogue CATALOGUE_JSON [--check-canonical-cores]".into(),
            );
        }
    };
    let limits = CatalogueLimits::default();
    let path = std::path::PathBuf::from(path);
    let mut bytes = Vec::new();
    std::fs::File::open(&path)?
        .take((limits.max_json_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limits.max_json_bytes {
        return Err(format!(
            "{} exceeds max_json_bytes = {}",
            path.display(),
            limits.max_json_bytes
        )
        .into());
    }
    let source = std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let catalogue = PhaseCatalogue::from_json(source, &limits, &ScalarArithmetic::default())?;
    let u = catalogue.usage();
    let entries = catalogue.entries().enumerate().map(|(index,e)| {
        let mut entry = serde_json::json!({"dimension":e.dimension(),"phase_table":e.phase_table(),"terms":e.terms().len()});
        if check_cores {
            let core = CanonicalPhaseCore::new(&phase_from_table(e.dimension(),e.phase_table())?, &PhaseCoreLimits::default())?;
            let (global,q) = core.clifford_correction();
            if core.phase_table() != e.phase_table() || global != 0 || q.constant() != 0
                || q.linear().iter().any(|c| *c != 0) || q.upper().iter().any(|row| !row.is_zero()) {
                return Err(format!("entry {index} has a valid decomposition but is not in canonical Clifford gauge").into());
            }
            entry["canonical_core"] = serde_json::json!(true);
        }
        Ok(entry)
    }).collect::<Result<Vec<_>,Box<dyn std::error::Error>>>()?;
    let mut result = serde_json::json!({"schema_version":1,"status":"certified","source_sha256":format!("{:x}",Sha256::digest(&bytes)),
        "usage":{"entries":u.entries,"table_values":u.table_values,"terms":u.terms,"constraint_rows":u.constraint_rows,"parity_rows":u.parity_rows,"verification_work":u.verification_work.to_string()},"entries":entries});
    if check_cores {
        result["canonical_cores_checked"] = serde_json::json!(true);
    }
    println!("{result}");
    Ok(())
}
