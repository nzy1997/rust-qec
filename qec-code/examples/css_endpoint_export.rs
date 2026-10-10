//! Export fixed-premise QRM64/Steane CSS endpoint tables in the official joint basis.
//! Usage: cargo run -p qec-code --no-default-features --example css_endpoint_export
//!        -- /path/to/pinned/joint.json /fresh/external/output
//! No circuit gates or physical cleanup are emitted. Output and input are sealed.
use qec_code::{
    Pauli,
    css_endpoint::{CssEndpointDecoder, EndpointLimits, ErrorSignature, PlaneTable},
    phased_pauli::{Phase, PhasedPauli, SignedStabilizerGroup},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

const INPUT_SHA256: &str = "3e4f670909cac4b60a1ea9d18316f0c8a8809f4b901a809d77e5d5d36881d618";
const MAX_OUTPUT_BYTES: usize = 20 * 1024 * 1024;
#[derive(Deserialize)]
struct Joint {
    n: usize,
    rank: usize,
    k: usize,
    checks: Vec<String>,
    lx: Vec<String>,
    lz: Vec<String>,
    columns: Vec<(u64, u64)>,
    #[serde(rename = "Steane_upstream_coordinate_permutation")]
    steane_permutation: Vec<usize>,
}
fn seal(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn parse(s: &str) -> Result<PhasedPauli, Box<dyn Error>> {
    let mut x = Vec::new();
    let mut z = Vec::new();
    let mut ny = 0;
    for ch in s.chars() {
        let (a, b) = match ch {
            'I' | '_' => (0, 0),
            'X' => (1, 0),
            'Z' => (0, 1),
            'Y' => {
                ny += 1;
                (1, 1)
            }
            _ => return Err("invalid positive Pauli string".into()),
        };
        x.push(a);
        z.push(b);
    }
    Ok(PhasedPauli::new(
        Pauli::from_xz_bits(x, z)?,
        Phase::from_exponent(ny),
    ))
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
fn sig_json(s: &ErrorSignature) -> Value {
    json!([format!("{:x}", s.syndrome), format!("{:x}", s.logical)])
}
fn table_json(t: &PlaneTable) -> Value {
    json!({"radius":t.radius(),"enumerated_masks":t.enumerated_masks(),"covered_syndromes":t.entries().len(),"rows":t.entries().iter().map(|(s,e)|[format!("{s:x}"),format!("{:x}",e.mask),format!("{:x}",e.logical)]).collect::<Vec<_>>()})
}
fn export_block(
    j: &Joint,
    start: usize,
    n: usize,
    check_range: std::ops::Range<usize>,
    logical_range: std::ops::Range<usize>,
    radius: usize,
    cap: usize,
) -> Result<Value, Box<dyn Error>> {
    let restriction = |s: &str| -> Result<PhasedPauli, Box<dyn Error>> {
        if s.len() != 71
            || !s[..start]
                .chars()
                .chain(s[start + n..].chars())
                .all(|c| c == 'I' || c == '_')
        {
            return Err("operator leaves declared block".into());
        }
        parse(&s[start..start + n])
    };
    let checks = check_range
        .clone()
        .map(|i| restriction(&j.checks[i]))
        .collect::<Result<Vec<_>, _>>()?;
    let g = SignedStabilizerGroup::new(n, checks)?;
    let lx = logical_range
        .clone()
        .map(|i| restriction(&j.lx[i]))
        .collect::<Result<Vec<_>, _>>()?;
    let lz = logical_range
        .clone()
        .map(|i| restriction(&j.lz[i]))
        .collect::<Result<Vec<_>, _>>()?;
    let b = g.validate_logical_basis(lx, lz)?;
    let d = CssEndpointDecoder::new(
        &g,
        &b,
        radius,
        EndpointLimits {
            max_physical_qubits: n,
            max_masks_per_plane: cap,
        },
    )?;
    // Re-embed ALL block columns in the independently verified joint columns.
    let logical_positions = logical_range
        .clone()
        .chain(logical_range.clone().map(|i| 21 + i))
        .collect::<Vec<_>>();
    for (axis, cols) in [d.x_columns(), d.z_columns()].iter().enumerate() {
        for (q, s) in cols.iter().enumerate() {
            let js = s.syndrome << check_range.start;
            let jl = logical_positions
                .iter()
                .enumerate()
                .fold(0u64, |v, (i, pos)| {
                    v | ((((s.logical >> i) & 1) as u64) << pos)
                });
            if j.columns[71 * axis + start + q] != (js as u64, jl) {
                return Err("restricted column/joint map mismatch".into());
            }
        }
    }
    Ok(
        json!({"n":n,"k":b.k(),"rank":g.rank(),"basis_phase_exponents":g.generators().iter().map(|p|p.phase().exponent()).collect::<Vec<_>>(),"checks":d.checks().iter().map(text).collect::<Vec<_>>(),"logical_x":d.logicals().logical_x().iter().map(text).collect::<Vec<_>>(),"logical_z":d.logicals().logical_z().iter().map(text).collect::<Vec<_>>(),"joint_physical_coordinates":(start..start+n).collect::<Vec<_>>(),"joint_check_indices":check_range.collect::<Vec<_>>(),"joint_logical_column_bits":logical_positions,"x_columns":d.x_columns().iter().map(sig_json).collect::<Vec<_>>(),"z_columns":d.z_columns().iter().map(sig_json).collect::<Vec<_>>(),"x_table":table_json(d.x_table()),"z_table":table_json(d.z_table())}),
    )
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("expected pinned_joint.json and fresh_external_output".into());
    }
    let input = &args[0];
    let out = &args[1];
    if fs::metadata(input)?.len() > 32 * 1024 {
        return Err("input exceeds32KiB".into());
    }
    let bytes = fs::read(input)?;
    if seal(&bytes) != INPUT_SHA256 {
        return Err("input pin mismatch".into());
    }
    let j: Joint = serde_json::from_slice(&bytes)?;
    if (
        j.n,
        j.rank,
        j.k,
        j.checks.len(),
        j.lx.len(),
        j.lz.len(),
        j.columns.len(),
    ) != (71, 50, 21, 50, 21, 21, 142)
    {
        return Err("joint shape mismatch".into());
    }
    let checks = j
        .checks
        .iter()
        .map(|s| parse(s))
        .collect::<Result<Vec<_>, _>>()?;
    let g = SignedStabilizerGroup::new(71, checks)?;
    let b = g.validate_logical_basis(
        j.lx.iter()
            .map(|s| parse(s))
            .collect::<Result<Vec<_>, _>>()?,
        j.lz.iter()
            .map(|s| parse(s))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    if g.rank() != 50 {
        return Err("joint rank mismatch".into());
    }
    for axis in 0..2 {
        for q in 0..71 {
            let mut bits = vec![0; 71];
            bits[q] = 1;
            let p = PhasedPauli::new(
                if axis == 0 {
                    Pauli::from_xz_bits(bits, vec![0; 71])?
                } else {
                    Pauli::from_xz_bits(vec![0; 71], bits)?
                },
                Phase::PlusOne,
            );
            let syndrome = g
                .generators()
                .iter()
                .enumerate()
                .try_fold(0u64, |s, (i, v)| {
                    Ok::<_, Box<dyn Error>>(s | (u64::from(!p.commutes_with(v)?) << i))
                })?;
            let logical = b
                .logical_x()
                .iter()
                .chain(b.logical_z())
                .enumerate()
                .try_fold(0u64, |s, (i, v)| {
                    Ok::<_, Box<dyn Error>>(s | (u64::from(!p.commutes_with(v)?) << i))
                })?;
            if j.columns[axis * 71 + q] != (syndrome, logical) {
                return Err("official joint signature mismatch".into());
            }
        }
    }
    let source = include_bytes!("css_endpoint_export.rs");
    let value = json!({"schema":"qec-code.css-endpoint.v1","diagnostic_only":true,"scope":"Fixed upstream basis/table export; no physical recovery, circuit/FT2/fullsource/failure/advantage certificate","signature_convention":"syndrome bit i anticommutes with declared check i; logical bits0..k anticommute with logicalX, bitsk..2k anticommute with logicalZ","table_row_convention":"[syndrome_hex,positive_binary_plane_mask_hex,logical_hex]","tie_policy":"minimum weight then lexicographic increasing-qubit support list","uncovered":"endpoint failure; no invented correction","input_sha256":INPUT_SHA256,"example_source_sha256":seal(source),"library_source_sha256":seal(include_bytes!("../src/css_endpoint.rs")),"Steane_upstream_coordinate_permutation":j.steane_permutation,"blocks":{"steane":export_block(&j,0,7,0..6,0..1,1,8)?,"qrm64":export_block(&j,7,64,6..50,1..21,3,43_745)?}});
    let payload = serde_json::to_vec(&value)?;
    let manifest = json!({"endpoint_tables_sha256":seal(&payload),"endpoint_tables_bytes":payload.len(),"input_sha256":INPUT_SHA256,"example_source_sha256":seal(source),"library_source_sha256":seal(include_bytes!("../src/css_endpoint.rs")),"masks_before_dedup":{"qrm64_per_plane":43745,"qrm64_total":87490,"steane_per_plane":8},"diagnostic_only":true});
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    if payload.len() + manifest_bytes.len() > MAX_OUTPUT_BYTES {
        return Err("combined output exceeds 20 MiB".into());
    }
    let parent = out.parent().unwrap_or(Path::new("."));
    let resolved_parent = fs::canonicalize(parent)?;
    let resolved_out = resolved_parent.join(out.file_name().ok_or("missing output name")?);
    let checkout = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())?;
    if resolved_out.starts_with(&checkout)
        || checkout.starts_with(&resolved_out)
        || input.canonicalize()?.starts_with(&resolved_out)
    {
        return Err("output aliases protected input/checkout".into());
    }
    fs::create_dir(&resolved_out)?; // Existing destinations and symlinks reject.
    let table = resolved_out.join("endpoint-tables.json");
    fs::write(&table, &payload)?;
    fs::write(resolved_out.join("manifest.json"), manifest_bytes)?;
    println!("{}", serde_json::to_string(&manifest)?);
    Ok(())
}
