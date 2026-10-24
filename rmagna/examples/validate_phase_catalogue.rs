//! Certify a bounded external phase catalogue without compiling quantum tasks.
use rmagna::catalogue::{CatalogueLimits, PhaseCatalogue};
use rmagna::scalar::ScalarArithmetic;
use sha2::{Digest, Sha256};
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: validate_phase_catalogue CATALOGUE_JSON")?;
    if args.next().is_some() {
        return Err("usage: validate_phase_catalogue CATALOGUE_JSON".into());
    }
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
    println!(
        "{}",
        serde_json::json!({"schema_version":1,"status":"certified","source_sha256":format!("{:x}",Sha256::digest(&bytes)),
        "usage":{"entries":u.entries,"table_values":u.table_values,"terms":u.terms,"constraint_rows":u.constraint_rows,"parity_rows":u.parity_rows,"verification_work":u.verification_work.to_string()},
        "entries":catalogue.entries().map(|e|serde_json::json!({"dimension":e.dimension(),"phase_table":e.phase_table(),"terms":e.terms().len()})).collect::<Vec<_>>() })
    );
    Ok(())
}
