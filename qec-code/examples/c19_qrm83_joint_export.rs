//! Bounded official C19/QRM64 tensor-interface export. No circuit certification.
use qec_code::{
    Pauli,
    css::{CssCode, SparseRowsMatrix},
    family_contract::{Color666FamilySpec, Color666Layout, CssFamilySpec, construct_css},
    phased_pauli::{Phase, PhasedPauli, SignedStabilizerGroup},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};
const QRM_PIN: &str = "da3e317c273c5bc5e654ec7269f0a74801498c7da4773139ba50817aeacc9ec9";
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn positive(p: Pauli) -> PhasedPauli {
    let n = p
        .x_bits()
        .iter()
        .zip(p.z_bits())
        .filter(|(x, z)| **x == 1 && **z == 1)
        .count();
    PhasedPauli::new(p, Phase::from_exponent((n % 4) as u8))
}
fn parse(s: &str) -> Result<PhasedPauli, Box<dyn Error>> {
    if s.len() != 64 {
        return Err("QRM width".into());
    }
    let mut x = Vec::new();
    let mut z = Vec::new();
    for c in s.chars() {
        let (a, b) = match c {
            'I' | '_' => (0, 0),
            'X' => (1, 0),
            'Z' => (0, 1),
            'Y' => (1, 1),
            _ => return Err("Pauli alphabet".into()),
        };
        x.push(a);
        z.push(b);
    }
    Ok(positive(Pauli::from_xz_bits(x, z)?))
}
fn text(p: &PhasedPauli) -> String {
    p.support()
        .x_bits()
        .iter()
        .zip(p.support().z_bits())
        .map(|(x, z)| match (x, z) {
            (0, 0) => 'I',
            (1, 0) => 'X',
            (0, 1) => 'Z',
            _ => 'Y',
        })
        .collect()
}
fn embed(p: &PhasedPauli, start: usize) -> Result<PhasedPauli, Box<dyn Error>> {
    let n = p.support().x_bits().len();
    if start + n > 83 {
        return Err("embed bounds".into());
    }
    let mut x = vec![0; 83];
    let mut z = vec![0; 83];
    x[start..start + n].copy_from_slice(p.support().x_bits());
    z[start..start + n].copy_from_slice(p.support().z_bits());
    Ok(PhasedPauli::new(Pauli::from_xz_bits(x, z)?, p.phase()))
}
fn read_ops(v: &Value, key: &str) -> Result<Vec<PhasedPauli>, Box<dyn Error>> {
    v[key]
        .as_array()
        .ok_or("array")?
        .iter()
        .map(|x| parse(x.as_str().ok_or("string")?))
        .collect()
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 1 {
        return Err("expected fresh direct /tmp/c19-qrm83-export-*".into());
    }
    let out = PathBuf::from(&args[0]);
    if out.parent() != Some(Path::new("/tmp"))
        || !out
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("c19-qrm83-export-"))
        || fs::symlink_metadata(&out).is_ok()
    {
        return Err("exclusive direct/tmp output".into());
    }
    let qpath = Path::new("/tmp/qec-css-endpoint-export-20261010-v2/endpoint-tables.json");
    if fs::metadata(qpath)?.len() > 20 * 1024 * 1024 {
        return Err("input cap".into());
    }
    let qb = fs::read(qpath)?;
    if sha(&qb) != QRM_PIN {
        return Err("QRM pin".into());
    }
    let qj: Value = serde_json::from_slice(&qb)?;
    let q = &qj["blocks"]["qrm64"];
    if (q["n"].as_u64(), q["k"].as_u64(), q["rank"].as_u64()) != (Some(64), Some(20), Some(44)) {
        return Err("QRM shape".into());
    }
    let qchecks = read_ops(q, "checks")?;
    let qlx = read_ops(q, "logical_x")?;
    let qlz = read_ops(q, "logical_z")?;
    let qg = SignedStabilizerGroup::new(64, qchecks.clone())?;
    let qbasis = qg.validate_logical_basis(qlx.clone(), qlz.clone())?;
    if qg.rank() != 44 || qbasis.k() != 20 {
        return Err("QRM official basis".into());
    }
    let c = construct_css(
        CssFamilySpec::Color666(Color666FamilySpec {
            distance: 5,
            layout: Color666Layout::Triangular,
        })
        .into(),
    )?;
    let hx = SparseRowsMatrix::new(19, c.checks.h_x.clone())?.to_dense_rows();
    let hz = SparseRowsMatrix::new(19, c.checks.h_z.clone())?.to_dense_rows();
    let css = CssCode::from_hx_hz(hx.clone(), hz.clone())?;
    let mut cc = Vec::new();
    for r in hx {
        cc.push(positive(Pauli::from_xz_bits(r, vec![0; 19])?));
    }
    for r in hz {
        cc.push(positive(Pauli::from_xz_bits(vec![0; 19], r)?));
    }
    let cg = SignedStabilizerGroup::new(19, cc.clone())?;
    let cb = css.code().canonical_logical_basis()?;
    let cb = cg.validate_logical_basis(
        cb.logical_x.into_iter().map(positive).collect(),
        cb.logical_z.into_iter().map(positive).collect(),
    )?;
    if cg.rank() != 18 || cb.k() != 1 {
        return Err("C19 shape".into());
    }
    let mut checks = cc
        .iter()
        .map(|p| embed(p, 0))
        .collect::<Result<Vec<_>, _>>()?;
    checks.extend(
        qchecks
            .iter()
            .map(|p| embed(p, 19))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let mut lx = cb
        .logical_x()
        .iter()
        .map(|p| embed(p, 0))
        .collect::<Result<Vec<_>, _>>()?;
    lx.extend(
        qlx.iter()
            .map(|p| embed(p, 19))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let mut lz = cb
        .logical_z()
        .iter()
        .map(|p| embed(p, 0))
        .collect::<Result<Vec<_>, _>>()?;
    lz.extend(
        qlz.iter()
            .map(|p| embed(p, 19))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let g = SignedStabilizerGroup::new(83, checks.clone())?;
    let basis = g.validate_logical_basis(lx, lz)?;
    if g.rank() != 62 || basis.k() != 21 {
        return Err("official tensor shape/basis".into());
    }
    let mut cols = Vec::new();
    for ax in 0..2 {
        for wire in 0..83 {
            let mut p = vec![0; 83];
            p[wire] = 1;
            let p = positive(if ax == 0 {
                Pauli::from_xz_bits(p, vec![0; 83])?
            } else {
                Pauli::from_xz_bits(vec![0; 83], p)?
            });
            let mut sy = 0u128;
            for (i, c) in checks.iter().enumerate() {
                sy |= u128::from(!p.commutes_with(c)?) << i;
            }
            let mut lo = 0u128;
            for (i, l) in basis
                .logical_x()
                .iter()
                .chain(basis.logical_z())
                .enumerate()
            {
                lo |= u128::from(!p.commutes_with(l)?) << i;
            }
            cols.push(json!([format!("{sy:x}"), format!("{lo:x}")]));
        }
    }
    let source = include_bytes!("c19_qrm83_joint_export.rs");
    let v = json!({"schema":"qec-code.c19-qrm83-joint.v1","n":83,"rank":62,"k":21,"all_logicals_unknown":true,"checks":checks.iter().map(text).collect::<Vec<_>>(),"check_phase_exponents":checks.iter().map(|p|p.phase().exponent()).collect::<Vec<_>>(),"logical_x":basis.logical_x().iter().map(text).collect::<Vec<_>>(),"logical_z":basis.logical_z().iter().map(text).collect::<Vec<_>>(),"columns_X_then_Z":cols,"C19_data":(0..19).collect::<Vec<_>>(),"QRM_data":(19..83).collect::<Vec<_>>(),"C19_checks":(0..18).collect::<Vec<_>>(),"QRM_checks":(18..62).collect::<Vec<_>>(),"logical_order":"controller0; QRMslots0..19 as joint1..20","signature_order":"62checks;21antiLX;21antiLZ","input_QRM_export_sha256":QRM_PIN,"upstream_library_commit":"0e266e174a98e60d1f75cd3c4acca382cb34f46a","example_source_sha256":sha(source),"scope":"Literal official operator tensor embedding and official signed group/logical validation. No circuit/FT/fullsource/noise certificate."});
    let bytes = serde_json::to_vec(&v)?;
    if bytes.len() > 256 * 1024 {
        return Err("output cap".into());
    }
    fs::create_dir(&out)?;
    fs::write(out.join("joint.json"), &bytes)?;
    fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(
            &json!({"source_sha256":sha(source),"joint_sha256":sha(&bytes),"joint_bytes":bytes.len(),"n_rank_k":[83,62,21],"input_sha256":QRM_PIN}),
        )?,
    )?;
    println!("official C19/QRM83 export complete");
    Ok(())
}
