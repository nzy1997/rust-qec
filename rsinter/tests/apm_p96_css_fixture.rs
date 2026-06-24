use qec_code::binary::try_binary_rank;
use qec_code::cli::{Cli, CodeCommands, Commands, CssArgs, CssMatrixKind, run};
use qec_code::css::{SparseRowsMatrix, sparse_rows_matrix_from_json_str};

const APM_P96_CODE_ID: &str = "apm_kasai:p=96";
const APM_P96_NUM_QUBITS: usize = 1152;
const APM_P96_LOGICALS: usize = 580;
const APM_P96_HX_JSON: &str = include_str!("fixtures/css/apm_p96_hx.json");
const APM_P96_HZ_JSON: &str = include_str!("fixtures/css/apm_p96_hz.json");

#[test]
fn apm_p96_css_fixture_has_580_logicals() {
    let hx_export = qec_code_css_stdout(CssMatrixKind::Hx);
    let hz_export = qec_code_css_stdout(CssMatrixKind::Hz);
    assert_eq!(APM_P96_HX_JSON, hx_export);
    assert_eq!(APM_P96_HZ_JSON, hz_export);

    let hx = parse_sparse_rows(APM_P96_HX_JSON);
    let hz = parse_sparse_rows(APM_P96_HZ_JSON);
    assert_eq!(hx.num_cols(), APM_P96_NUM_QUBITS);
    assert_eq!(hz.num_cols(), APM_P96_NUM_QUBITS);

    let hx_dense = hx.to_dense_rows();
    let hz_dense = hz.to_dense_rows();
    let rank_x = try_binary_rank(&hx_dense).unwrap();
    let rank_z = try_binary_rank(&hz_dense).unwrap();
    let rank_sum = rank_x + rank_z;
    let logicals = APM_P96_NUM_QUBITS
        .checked_sub(rank_sum)
        .expect("rank_x + rank_z must not exceed n");
    assert_eq!(logicals, APM_P96_LOGICALS);
    assert!(sparse_rows_are_orthogonal(&hx, &hz));

    let mut corrupted_hz_rows = hz.rows().to_vec();
    assert_eq!(corrupted_hz_rows[0][0], 69);
    corrupted_hz_rows[0][0] = 0;
    let corrupted_hz = SparseRowsMatrix::new(hz.num_cols(), corrupted_hz_rows).unwrap();

    assert_ne!(
        format!("{}\n", corrupted_hz.to_json_string()),
        hz_export,
        "changed Hz support should no longer match the qec-code export"
    );
    assert!(
        !sparse_rows_are_orthogonal(&hx, &corrupted_hz),
        "changed Hz support should also break CSS orthogonality"
    );
}

fn qec_code_css_stdout(matrix: CssMatrixKind) -> String {
    let output = run(Cli {
        command: Commands::Code {
            command: CodeCommands::Css(CssArgs::export(APM_P96_CODE_ID.to_owned(), matrix)),
        },
    })
    .unwrap();
    format!("{output}\n")
}

fn parse_sparse_rows(input: &str) -> SparseRowsMatrix {
    sparse_rows_matrix_from_json_str(input).unwrap()
}

fn sparse_rows_are_orthogonal(hx: &SparseRowsMatrix, hz: &SparseRowsMatrix) -> bool {
    for x_row in hx.rows() {
        for z_row in hz.rows() {
            let overlap = x_row
                .iter()
                .filter(|x_col| z_row.contains(x_col))
                .count();
            if overlap % 2 != 0 {
                return false;
            }
        }
    }
    true
}
