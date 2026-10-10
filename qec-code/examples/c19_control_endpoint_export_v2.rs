//! Fixed C19 color_666:d=5 bounded endpoint algebra export, diagnostic only.
use qec_code::{
    Pauli,
    css::{CssCode, SparseRowsMatrix},
    css_endpoint::{CssEndpointDecoder, EndpointLimits, PlaneTable},
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
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
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
fn positive(p: Pauli) -> PhasedPauli {
    let ny = p
        .x_bits()
        .iter()
        .zip(p.z_bits())
        .filter(|(x, z)| **x == 1 && **z == 1)
        .count();
    PhasedPauli::new(p, Phase::from_exponent((ny % 4) as u8))
}
fn table(t: &PlaneTable) -> Value {
    json!({"radius":t.radius(),"enumerated_masks":t.enumerated_masks(),"covered_syndromes":t.entries().len(),"rows":t.entries().iter().map(|(s,e)|[format!("{s:x}"),format!("{:x}",e.mask),format!("{:x}",e.logical)]).collect::<Vec<_>>()})
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 1 {
        return Err("expected fresh direct /tmp/c19-control-export-* output".into());
    };
    let raw = PathBuf::from(&args[0]);
    let name = raw
        .file_name()
        .ok_or("no output name")?
        .to_str()
        .ok_or("nonutf8 output")?;
    if raw.parent() != Some(Path::new("/tmp"))
        || !name.starts_with("c19-control-export-")
        || fs::symlink_metadata(&raw).is_ok()
    {
        return Err(
            "output must be new direct /tmp/c19-control-export-* and not alias an existing path"
                .into(),
        );
    }
    let r = construct_css(
        CssFamilySpec::Color666(Color666FamilySpec {
            distance: 5,
            layout: Color666Layout::Triangular,
        })
        .into(),
    )?;
    if (
        r.stats.n,
        r.stats.k,
        r.stats.rank_x,
        r.stats.rank_z,
        r.stats.d_x,
        r.stats.d_z,
    ) != (19, 1, 9, 9, Some(5), Some(5))
    {
        return Err("C19 shape/distances mismatch".into());
    }
    let hx = SparseRowsMatrix::new(19, r.checks.h_x.clone())?.to_dense_rows();
    let hz = SparseRowsMatrix::new(19, r.checks.h_z.clone())?.to_dense_rows();
    let css = CssCode::from_hx_hz(hx.clone(), hz.clone())?;
    let mut generators = Vec::new();
    for row in &hx {
        generators.push(positive(Pauli::from_xz_bits(row.clone(), vec![0; 19])?))
    }
    for row in &hz {
        generators.push(positive(Pauli::from_xz_bits(vec![0; 19], row.clone())?))
    }
    let sg = SignedStabilizerGroup::new(19, generators)?;
    let basis = css.code().canonical_logical_basis()?;
    let lb = sg.validate_logical_basis(
        basis.logical_x.into_iter().map(positive).collect(),
        basis.logical_z.into_iter().map(positive).collect(),
    )?;
    let d = CssEndpointDecoder::new(
        &sg,
        &lb,
        2,
        EndpointLimits {
            max_physical_qubits: 19,
            max_masks_per_plane: 191,
        },
    )?;
    let allx = positive(Pauli::from_xz_bits(vec![1; 19], vec![0; 19])?);
    let allz = positive(Pauli::from_xz_bits(vec![0; 19], vec![1; 19])?);
    let all = sg.validate_logical_basis(vec![allx], vec![allz])?;
    let sig = |s: &qec_code::css_endpoint::ErrorSignature| {
        json!([format!("{:x}", s.syndrome), format!("{:x}", s.logical)])
    };
    let source = include_bytes!("c19_control_endpoint_export_v2.rs");
    let value = json!({"schema":"qec-code.c19-control-endpoint.v1","diagnostic_only":true,"scope":"Fixed official color_666 d5 algebra/table; no physical correction or circuit/FT2/fullsource/noise advantage certificate","upstream_commit":"0e266e174a98e60d1f75cd3c4acca382cb34f46a","construction":r,"n":19,"k":1,"rank":sg.rank(),"coordinates":(0..19).collect::<Vec<_>>(),"all_patch_logicals_unknown":true,"checks":d.checks().iter().map(text).collect::<Vec<_>>(),"basis_phase_exponents":sg.generators().iter().map(|g|g.phase().exponent()).collect::<Vec<_>>(),"logical_x":d.logicals().logical_x().iter().map(text).collect::<Vec<_>>(),"logical_z":d.logicals().logical_z().iter().map(text).collect::<Vec<_>>(),"all19_validated_logical_x":all.logical_x().iter().map(text).collect::<Vec<_>>(),"all19_validated_logical_z":all.logical_z().iter().map(text).collect::<Vec<_>>(),"x_columns":d.x_columns().iter().map(sig).collect::<Vec<_>>(),"z_columns":d.z_columns().iter().map(sig).collect::<Vec<_>>(),"x_table":table(d.x_table()),"z_table":table(d.z_table()),"example_source_sha256":sha(source),"endpoint_library_source_sha256":sha(include_bytes!("../src/css_endpoint.rs")),"color666_source_sha256":sha(include_bytes!("../src/codes/color_666.rs")),"table_row_convention":"syndrome_hex,positive_plane_mask_hex,logical_hex; bits0 antiLX(Zlogical), bit1 antiLZ(Xlogical)","tie_policy":"minimum weight then lexicographic increasing support","uncovered":"diagnostic failure, no invented physical recovery"});
    let payload = serde_json::to_vec(&value)?;
    if payload.len() > 256 * 1024 {
        return Err("256KiB output cap".into());
    };
    let manifest = serde_json::to_vec_pretty(
        &json!({"endpoint_tables_sha256":sha(&payload),"endpoint_tables_bytes":payload.len(),"source_sha256":sha(source),"upstream_commit":"0e266e174a98e60d1f75cd3c4acca382cb34f46a","radius":2,"masks_per_plane":191,"diagnostic_only":true}),
    )?;
    fs::create_dir(&raw)?;
    fs::write(raw.join("endpoint-tables.json"), payload)?;
    fs::write(raw.join("manifest.json"), manifest)?;
    println!("C19 fixed export complete");
    Ok(())
}
