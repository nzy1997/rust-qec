const SX_LABELS: [&str; 7] = ["idle", "1", "4", "3", "5", "0", "2"];
const SZ_LABELS: [&str; 7] = ["3", "5", "0", "1", "2", "4", "idle"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BivariateBicycleParams {
    pub ell: usize,
    pub m: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub b1: usize,
    pub b2: usize,
    pub b3: usize,
}

impl BivariateBicycleParams {
    pub fn upstream_default() -> Self {
        Self {
            ell: 12,
            m: 6,
            a1: 3,
            a2: 1,
            a3: 2,
            b1: 3,
            b2: 1,
            b3: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BbCode {
    params: BivariateBicycleParams,
    n2: usize,
    k: usize,
    x_checks: Vec<usize>,
    z_checks: Vec<usize>,
    data_qubits: Vec<usize>,
    hx_rows: Vec<Vec<usize>>,
    hz_rows: Vec<Vec<usize>>,
    logical_x_rows: Vec<Vec<usize>>,
    logical_z_rows: Vec<Vec<usize>>,
    x_cnot_targets: Vec<[usize; 6]>,
    z_cnot_targets: Vec<[usize; 6]>,
}

impl BbCode {
    pub fn ell(&self) -> usize {
        self.params.ell
    }

    pub fn m(&self) -> usize {
        self.params.m
    }

    pub fn n2(&self) -> usize {
        self.n2
    }

    pub fn n(&self) -> usize {
        2 * self.n2
    }

    pub fn k(&self) -> usize {
        self.k
    }

    pub fn x_checks(&self) -> &[usize] {
        &self.x_checks
    }

    pub fn z_checks(&self) -> &[usize] {
        &self.z_checks
    }

    pub fn data_qubits(&self) -> &[usize] {
        &self.data_qubits
    }

    pub fn num_circuit_qubits(&self) -> usize {
        4 * self.n2
    }

    pub fn hx_rows(&self) -> &[Vec<usize>] {
        &self.hx_rows
    }

    pub fn hz_rows(&self) -> &[Vec<usize>] {
        &self.hz_rows
    }

    pub fn logical_x_rows(&self) -> &[Vec<usize>] {
        &self.logical_x_rows
    }

    pub fn logical_z_rows(&self) -> &[Vec<usize>] {
        &self.logical_z_rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationKind {
    Idle,
    Cnot,
    PrepX,
    PrepZ,
    MeasX,
    MeasZ,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    kind: OperationKind,
    qubits: Vec<usize>,
}

impl Operation {
    fn new(kind: OperationKind, qubits: Vec<usize>) -> Self {
        Self { kind, qubits }
    }

    pub fn kind(&self) -> OperationKind {
        self.kind
    }

    pub fn qubits(&self) -> &[usize] {
        &self.qubits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyndromeCycle {
    operations: Vec<Operation>,
    sx_labels: [&'static str; 7],
    sz_labels: [&'static str; 7],
}

impl SyndromeCycle {
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn count(&self, kind: OperationKind) -> usize {
        self.operations
            .iter()
            .filter(|operation| operation.kind == kind)
            .count()
    }

    pub fn sx_labels(&self) -> [&'static str; 7] {
        self.sx_labels
    }

    pub fn sz_labels(&self) -> [&'static str; 7] {
        self.sz_labels
    }
}

pub fn build_upstream_code() -> Result<BbCode, String> {
    let params = BivariateBicycleParams::upstream_default();
    let n2 = params.ell * params.m;
    let width = 2 * n2;
    let x_checks = (0..n2).collect::<Vec<_>>();
    let z_checks = (3 * n2..4 * n2).collect::<Vec<_>>();
    let data_qubits = (n2..3 * n2).collect::<Vec<_>>();
    let mut hx_rows = Vec::with_capacity(n2);
    let mut hz_rows = Vec::with_capacity(n2);
    let mut x_cnot_targets = Vec::with_capacity(n2);
    let mut z_cnot_targets = Vec::with_capacity(n2);

    for row in 0..n2 {
        let x_slots = x_row_slots_local(&params, row);
        let z_slots = z_row_slots_local(&params, row);

        let mut hx_row = x_slots.to_vec();
        hx_row.sort_unstable();
        hx_rows.push(hx_row);
        x_cnot_targets.push(local_to_circuit_targets(&x_slots, n2));

        let mut hz_row = z_slots.to_vec();
        hz_row.sort_unstable();
        hz_rows.push(hz_row);
        z_cnot_targets.push(local_to_circuit_targets(&z_slots, n2));
    }

    let dense_hx = sparse_to_dense_rows(&hx_rows, width);
    let dense_hz = sparse_to_dense_rows(&hz_rows, width);

    let hx_rank = rank(&dense_hx);
    let hz_rank = rank(&dense_hz);
    let k = width
        .checked_sub(hx_rank + hz_rank)
        .ok_or_else(|| "bb144 rank computation underflowed".to_owned())?;

    let logical_x_rows = select_logical_rows(nullspace(&dense_hz, width), &dense_hx, k)
        .into_iter()
        .map(dense_to_sparse_row)
        .collect::<Vec<_>>();
    let logical_z_rows = select_logical_rows(nullspace(&dense_hx, width), &dense_hz, k)
        .into_iter()
        .map(dense_to_sparse_row)
        .collect::<Vec<_>>();

    if logical_x_rows.len() != k || logical_z_rows.len() != k {
        return Err(format!(
            "failed to extract {k} logical rows (x={}, z={})",
            logical_x_rows.len(),
            logical_z_rows.len()
        ));
    }

    Ok(BbCode {
        params,
        n2,
        k,
        x_checks,
        z_checks,
        data_qubits,
        hx_rows,
        hz_rows,
        logical_x_rows,
        logical_z_rows,
        x_cnot_targets,
        z_cnot_targets,
    })
}

pub fn build_syndrome_cycle(code: &BbCode) -> SyndromeCycle {
    let mut operations = Vec::with_capacity(1440);

    let round0_sz_slot = parse_schedule_slot(SZ_LABELS[0]).expect("round 0 must use a Z slot");
    let round6_sx_slot =
        parse_schedule_slot(SX_LABELS[6]).expect("round 6 must use an X slot");

    for &check in &code.x_checks {
        operations.push(Operation::new(OperationKind::PrepX, vec![check]));
    }
    for (row, &check) in code.z_checks.iter().enumerate() {
        operations.push(Operation::new(
            OperationKind::Cnot,
            vec![code.z_cnot_targets[row][round0_sz_slot], check],
        ));
    }
    append_idle_untouched_data(
        &mut operations,
        code,
        code.z_cnot_targets
            .iter()
            .map(|targets| targets[round0_sz_slot]),
    );

    for round in 1..6 {
        let sx_slot = parse_schedule_slot(SX_LABELS[round]).expect("middle rounds must use X");
        let sz_slot = parse_schedule_slot(SZ_LABELS[round]).expect("middle rounds must use Z");

        for (row, &check) in code.x_checks.iter().enumerate() {
            operations.push(Operation::new(
                OperationKind::Cnot,
                vec![check, code.x_cnot_targets[row][sx_slot]],
            ));
        }

        for (row, &check) in code.z_checks.iter().enumerate() {
            operations.push(Operation::new(
                OperationKind::Cnot,
                vec![code.z_cnot_targets[row][sz_slot], check],
            ));
        }
    }

    for &check in &code.z_checks {
        operations.push(Operation::new(OperationKind::MeasZ, vec![check]));
    }
    for (row, &check) in code.x_checks.iter().enumerate() {
        operations.push(Operation::new(
            OperationKind::Cnot,
            vec![check, code.x_cnot_targets[row][round6_sx_slot]],
        ));
    }
    append_idle_untouched_data(
        &mut operations,
        code,
        code.x_cnot_targets
            .iter()
            .map(|targets| targets[round6_sx_slot]),
    );

    for &data in &code.data_qubits {
        operations.push(Operation::new(OperationKind::Idle, vec![data]));
    }
    for &check in &code.x_checks {
        operations.push(Operation::new(OperationKind::MeasX, vec![check]));
    }
    for &check in &code.z_checks {
        operations.push(Operation::new(OperationKind::PrepZ, vec![check]));
    }

    SyndromeCycle {
        operations,
        sx_labels: SX_LABELS,
        sz_labels: SZ_LABELS,
    }
}

fn x_row_slots_local(params: &BivariateBicycleParams, row: usize) -> [usize; 6] {
    let n2 = params.ell * params.m;
    [
        shift_x(params, row, params.a1),
        shift_y(params, row, params.a2),
        shift_y(params, row, params.a3),
        n2 + shift_y(params, row, params.b1),
        n2 + shift_x(params, row, params.b2),
        n2 + shift_x(params, row, params.b3),
    ]
}

fn z_row_slots_local(params: &BivariateBicycleParams, row: usize) -> [usize; 6] {
    let n2 = params.ell * params.m;
    [
        shift_y_inverse(params, row, params.b1),
        shift_x_inverse(params, row, params.b2),
        shift_x_inverse(params, row, params.b3),
        n2 + shift_x_inverse(params, row, params.a1),
        n2 + shift_y_inverse(params, row, params.a2),
        n2 + shift_y_inverse(params, row, params.a3),
    ]
}

fn local_to_circuit_targets(local_targets: &[usize; 6], n2: usize) -> [usize; 6] {
    local_targets.map(|target| target + n2)
}

fn shift_x(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    ((x + shift) % params.ell) * params.m + y
}

fn shift_y(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    x * params.m + (y + shift) % params.m
}

fn shift_x_inverse(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    ((x + params.ell - shift % params.ell) % params.ell) * params.m + y
}

fn shift_y_inverse(params: &BivariateBicycleParams, row: usize, shift: usize) -> usize {
    let x = row / params.m;
    let y = row % params.m;
    x * params.m + (y + params.m - shift % params.m) % params.m
}

fn parse_schedule_slot(label: &str) -> Option<usize> {
    if label == "idle" {
        None
    } else {
        label.parse::<usize>().ok()
    }
}

fn append_idle_untouched_data(
    operations: &mut Vec<Operation>,
    code: &BbCode,
    touched_data: impl IntoIterator<Item = usize>,
) {
    let mut touched = vec![false; code.num_circuit_qubits()];
    for qubit in touched_data {
        touched[qubit] = true;
    }

    for &data in &code.data_qubits {
        if !touched[data] {
            operations.push(Operation::new(OperationKind::Idle, vec![data]));
        }
    }
}

fn sparse_to_dense_rows(rows: &[Vec<usize>], width: usize) -> Vec<Vec<u8>> {
    rows.iter()
        .map(|row| {
            let mut dense = vec![0u8; width];
            for &col in row {
                dense[col] ^= 1;
            }
            dense
        })
        .collect()
}

fn dense_to_sparse_row(row: Vec<u8>) -> Vec<usize> {
    row.into_iter()
        .enumerate()
        .filter_map(|(index, bit)| (bit == 1).then_some(index))
        .collect()
}

fn rank(rows: &[Vec<u8>]) -> usize {
    rref(rows).1.len()
}

fn rref(rows: &[Vec<u8>]) -> (Vec<Vec<u8>>, Vec<usize>) {
    if rows.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let width = rows[0].len();
    let mut reduced = rows.to_vec();
    let mut pivot_cols = Vec::new();
    let mut pivot_row = 0usize;

    for col in 0..width {
        let Some(found) = (pivot_row..reduced.len()).find(|&row| reduced[row][col] == 1) else {
            continue;
        };
        reduced.swap(pivot_row, found);

        for row in 0..reduced.len() {
            if row != pivot_row && reduced[row][col] == 1 {
                for entry in col..width {
                    reduced[row][entry] ^= reduced[pivot_row][entry];
                }
            }
        }

        pivot_cols.push(col);
        pivot_row += 1;
        if pivot_row == reduced.len() {
            break;
        }
    }

    (reduced, pivot_cols)
}

fn nullspace(rows: &[Vec<u8>], width: usize) -> Vec<Vec<u8>> {
    if rows.is_empty() {
        return (0..width)
            .map(|free_col| {
                let mut row = vec![0u8; width];
                row[free_col] = 1;
                row
            })
            .collect();
    }

    let (reduced, pivot_cols) = rref(rows);
    let mut is_pivot = vec![false; width];
    for &pivot in &pivot_cols {
        is_pivot[pivot] = true;
    }

    let mut basis = Vec::new();
    for free_col in 0..width {
        if is_pivot[free_col] {
            continue;
        }
        let mut vector = vec![0u8; width];
        vector[free_col] = 1;
        for (pivot_row, &pivot_col) in pivot_cols.iter().enumerate() {
            vector[pivot_col] = reduced[pivot_row][free_col];
        }
        basis.push(vector);
    }
    basis
}

fn in_row_span(span_rows: &[Vec<u8>], target: &[u8]) -> bool {
    if span_rows.is_empty() {
        return target.iter().all(|bit| *bit == 0);
    }

    rank(&{
        let mut augmented = span_rows.to_vec();
        augmented.push(target.to_vec());
        augmented
    }) == rank(span_rows)
}

fn select_logical_rows(
    candidates: Vec<Vec<u8>>,
    stabilizers: &[Vec<u8>],
    count: usize,
) -> Vec<Vec<u8>> {
    let mut span_rows = stabilizers.to_vec();
    let mut logicals = Vec::with_capacity(count);

    for candidate in candidates {
        if in_row_span(&span_rows, &candidate) {
            continue;
        }

        span_rows.push(candidate.clone());
        logicals.push(candidate);
        if logicals.len() == count {
            break;
        }
    }

    logicals
}
